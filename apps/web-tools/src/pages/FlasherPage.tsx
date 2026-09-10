import { useState, useCallback, useRef, useMemo, useEffect } from "react";
import { useParams, useNavigate } from "react-router";
import { ESPLoader, Transport } from "esptool-js";
import CryptoJS from "crypto-js";
import {
  Box,
  Card,
  Flex,
  Heading,
  Text,
  Select,
  Button,
  Callout,
  Separator,
  Progress,
  ScrollArea,
  Code,
  AlertDialog,
  Avatar,
} from "@radix-ui/themes";
import {
  InfoCircledIcon,
  ExclamationTriangleIcon,
  DownloadIcon,
} from "@radix-ui/react-icons";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useGithubReleases } from "../hooks/useGithubReleases";
import { usePrFirmware } from "../hooks/usePrFirmware";
import { useGithubPr } from "../hooks/useGithubPr";

const BAUD = 921600;

const BOARD_LABELS: Record<string, string> = {
  "ossm-alt": "OSSM Alt",
  "ossm-reference": "OSSM Reference Board",
  waveshare: "Waveshare ESP32-S3-RS485-CAN",
  "seeed-xiao": "Seeed XIAO ESP32S3",
};

type FlashState =
  | "idle"
  | "connecting"
  | "connected"
  | "flashing"
  | "done"
  | "error";

type FlashMode = "release" | "pr";

export default function FlasherPage() {
  const params = useParams<{
    release?: string;
    pr?: string;
    board?: string;
  }>();
  const navigate = useNavigate();

  const {
    releases,
    loading: releasesLoading,
    error: releasesError,
  } = useGithubReleases();

  const mode: FlashMode = params.pr ? "pr" : "release";
  const prNumber = params.pr ?? null;

  const {
    boards: prBoards,
    loading: prLoading,
    error: prError,
  } = usePrFirmware(prNumber);

  const { prInfo } = useGithubPr(prNumber);

  const [flashState, setFlashState] = useState<FlashState>("idle");
  const [progress, setProgress] = useState(0);
  const [logs, setLogs] = useState<string[]>([]);
  const logEndRef = useRef<HTMLDivElement>(null);

  const transportRef = useRef<Transport | null>(null);
  const espLoaderRef = useRef<ESPLoader | null>(null);
  const deviceRef = useRef<SerialPort | null>(null);

  const webSerialSupported =
    typeof navigator !== "undefined" && "serial" in navigator;

  useEffect(() => {
    return () => {
      transportRef.current?.disconnect().catch(() => {});
    };
  }, []);

  // Resolve selected release: use URL param if valid, otherwise fall back to first
  const selectedRelease = useMemo(() => {
    if (mode === "pr") return "";
    if (params.release && releases.some((r) => r.tag_name === params.release)) {
      return params.release;
    }
    return releases[0]?.tag_name ?? "";
  }, [mode, params.release, releases]);

  // Derive available boards from the selected release's .bin assets (release mode)
  const releaseBoards = useMemo(() => {
    const release = releases.find((r) => r.tag_name === selectedRelease);
    if (!release) return [];
    return release.assets
      .filter((a) => a.name.endsWith(".bin"))
      .map((a) => {
        const stem = a.name.replace(/\.bin$/, "");
        return {
          value: stem,
          label: BOARD_LABELS[stem] ?? a.name,
          assetId: a.id,
          size: a.size,
        };
      });
  }, [releases, selectedRelease]);

  // Unified board list for PR mode
  const prBoardOptions = useMemo(() => {
    return prBoards.map((b) => ({
      value: b.board,
      label: BOARD_LABELS[b.board] ?? b.board,
      size: b.size,
    }));
  }, [prBoards]);

  const boards = mode === "pr" ? prBoardOptions : releaseBoards;

  // Resolve selected board: use URL param if valid, otherwise empty (user must choose)
  const selectedBoard = useMemo(() => {
    if (params.board && boards.some((b) => b.value === params.board)) {
      return params.board;
    }
    return "";
  }, [params.board, boards]);

  // Keep URL in sync when release default is resolved
  useEffect(() => {
    if (mode === "pr") return;
    if (releasesLoading || releasesError || releases.length === 0) return;
    if (params.release !== selectedRelease && selectedRelease) {
      navigate(`/firmware/release/${selectedRelease}`, { replace: true });
    }
  }, [
    mode,
    releasesLoading,
    releasesError,
    releases,
    selectedRelease,
    params.release,
    navigate,
  ]);

  const handleReleaseChange = useCallback(
    (value: string) => {
      navigate(`/firmware/release/${value}`, { replace: true });
    },
    [navigate],
  );

  const handleBoardChange = useCallback(
    (value: string) => {
      if (mode === "pr") {
        navigate(`/firmware/pr/${prNumber}/${value}`, { replace: true });
      } else {
        navigate(`/firmware/release/${selectedRelease}/${value}`, {
          replace: true,
        });
      }
    },
    [mode, prNumber, selectedRelease, navigate],
  );

  const appendLog = useCallback((line: string) => {
    setLogs((prev) => [...prev, line]);
    requestAnimationFrame(() => {
      logEndRef.current?.scrollIntoView({ behavior: "smooth" });
    });
  }, []);

  const espLoaderTerminal = useRef({
    clean() {},
    writeLine(_data: string) {},
    write(_data: string) {},
  }).current;

  const handleConnect = useCallback(async () => {
    if (flashState === "connected" || flashState === "connecting") {
      if (transportRef.current) {
        await transportRef.current.disconnect();
      }
      transportRef.current = null;
      espLoaderRef.current = null;
      deviceRef.current = null;
      setFlashState("idle");
      setLogs([]);
      return;
    }

    try {
      setFlashState("connecting");
      appendLog("Requesting serial port...");

      const device = await navigator.serial.requestPort();
      deviceRef.current = device;

      const transport = new Transport(device);
      transportRef.current = transport;

      const espLoader = new ESPLoader({
        transport,
        baudrate: BAUD,
        terminal: espLoaderTerminal,
      });
      espLoaderRef.current = espLoader;

      const chip = await espLoader.main();
      appendLog(`Connected to ${chip}`);
      setFlashState("connected");
    } catch (err) {
      appendLog(
        `Connection failed: ${err instanceof Error ? err.message : err}`,
      );
      setFlashState("error");
    }
  }, [flashState, appendLog, espLoaderTerminal]);

  const handleFlash = useCallback(async () => {
    const espLoader = espLoaderRef.current;
    if (!espLoader || !selectedBoard) return;

    const board = boards.find((b) => b.value === selectedBoard);
    if (!board) {
      appendLog("No binary selected");
      setFlashState("error");
      return;
    }

    try {
      setFlashState("flashing");
      setProgress(0);

      const url =
        mode === "pr"
          ? `/api/pr-firmware/${prNumber}/${selectedBoard}`
          : `/api/github-asset/${(board as (typeof releaseBoards)[number]).assetId}`;

      appendLog(
        `Downloading ${selectedBoard}.bin (${formatBytes(board.size)})...`,
      );
      const response = await fetch(url);
      if (!response.ok) {
        throw new Error(`Download failed: ${response.status}`);
      }
      const arrayBuffer = await response.arrayBuffer();
      const binaryData = new Uint8Array(arrayBuffer);
      appendLog(`Downloaded ${arrayBuffer.byteLength} bytes`);

      appendLog("Flashing...");
      await espLoader.writeFlash({
        fileArray: [{ data: binaryData, address: 0 }],
        flashSize: "keep",
        flashMode: "keep",
        flashFreq: "keep",
        eraseAll: false,
        compress: true,
        reportProgress: (
          _fileIndex: number,
          written: number,
          total: number,
        ) => {
          setProgress(Math.round((written / total) * 100));
        },
        calculateMD5Hash: (image: Uint8Array) => {
          const wordArray = CryptoJS.lib.WordArray.create(image as unknown as number[]);
          return CryptoJS.MD5(wordArray).toString();
        },
      });

      appendLog("Resetting device...");
      await espLoader.after();
      appendLog("Flash complete!");
      setFlashState("done");
    } catch (err) {
      appendLog(`Flash failed: ${err instanceof Error ? err.message : err}`);
      setFlashState("error");
    }
  }, [boards, selectedBoard, mode, prNumber, releaseBoards, appendLog]);

  const downloadUrl = useMemo(() => {
    if (!selectedBoard) return null;
    if (mode === "pr") return `/api/pr-firmware/${prNumber}/${selectedBoard}`;
    const board = releaseBoards.find((b) => b.value === selectedBoard);
    if (!board) return null;
    return `/api/github-asset/${board.assetId}`;
  }, [mode, prNumber, selectedBoard, releaseBoards]);

  const isConnected =
    flashState === "connected" ||
    flashState === "flashing" ||
    flashState === "done";
  const isFlashing = flashState === "flashing";
  const sourceLoading = mode === "pr" ? prLoading : releasesLoading;
  const sourceError = mode === "pr" ? prError : releasesError;

  return (
    <Flex
      direction="column"
      align="center"
      style={{ flex: 1, overflow: "auto" }}
      p="4"
      gap="4"
    >
      <Box style={{ width: "100%", maxWidth: 600 }}>
        <Heading size="5" mb="4">
          Flash Firmware
        </Heading>

        {mode === "pr" && (
          <Callout.Root color="orange" mb="4">
            <Callout.Icon>
              <ExclamationTriangleIcon />
            </Callout.Icon>
            <Callout.Text>
              This is an unreviewed pre-release build from a pull request. It
              has not been tested or approved and could cause unexpected
              behaviour. Only flash this firmware if you know what you're doing.
            </Callout.Text>
          </Callout.Root>
        )}

        {!webSerialSupported && (
          <Callout.Root color="red" mb="4">
            <Callout.Icon>
              <ExclamationTriangleIcon />
            </Callout.Icon>
            <Callout.Text>
              Web Serial is not supported in this browser. Please use Chrome or
              Edge.
            </Callout.Text>
          </Callout.Root>
        )}

        {mode === "pr" && prNumber && prInfo && (
          <Card size="2" mb="4">
            <Flex direction="column" gap="2">
              <Flex align="center" gap="2">
                <Avatar
                  size="1"
                  src={prInfo.author.avatar_url}
                  fallback={prInfo.author.username[0]}
                  radius="full"
                  style={{ width: 16, height: 16, flexShrink: 0 }}
                />
                <Text size="1" color="gray" weight="medium">
                  <a
                    href={prInfo.author.html_url}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    {prInfo.author.username}
                  </a>{" "}
                  opened{" "}
                  <a
                    href={`https://github.com/ossm-rs/ossm/pull/${prNumber}`}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    PR #{prNumber}
                  </a>
                </Text>
              </Flex>
              <Heading size="4">{prInfo.title}</Heading>
              {prInfo.body && (
                <Flex direction="column" gap="1">
                  <Separator size="4" />
                  <Text size="2" color="gray" asChild>
                    <div>
                      <Markdown remarkPlugins={[remarkGfm]}>
                        {prInfo.body}
                      </Markdown>
                    </div>
                  </Text>
                </Flex>
              )}
            </Flex>
          </Card>
        )}

        <Card size="2">
          <Flex direction="column" gap="4">
            {mode === "release" && (
              <Flex direction="column" gap="2">
                <Text size="2" weight="medium" as="label">
                  Release
                </Text>
                {releasesLoading ? (
                  <Text size="2" color="gray">
                    Loading releases...
                  </Text>
                ) : releasesError ? (
                  <Callout.Root color="red" size="1">
                    <Callout.Icon>
                      <ExclamationTriangleIcon />
                    </Callout.Icon>
                    <Callout.Text>{releasesError}</Callout.Text>
                  </Callout.Root>
                ) : (
                  <Select.Root
                    value={selectedRelease}
                    onValueChange={handleReleaseChange}
                    disabled={isConnected}
                  >
                    <Select.Trigger style={{ width: "100%" }} />
                    <Select.Content>
                      {releases.map((r) => (
                        <Select.Item key={r.tag_name} value={r.tag_name}>
                          {r.tag_name}
                          {r.name && r.name !== r.tag_name
                            ? ` - ${r.name}`
                            : ""}
                        </Select.Item>
                      ))}
                    </Select.Content>
                  </Select.Root>
                )}
              </Flex>
            )}

            <Flex direction="column" gap="2">
              <Text size="2" weight="medium" as="label">
                Board
              </Text>
              {sourceLoading ? (
                <Text size="2" color="gray">
                  Loading firmware...
                </Text>
              ) : sourceError ? (
                <Callout.Root color="red" size="1">
                  <Callout.Icon>
                    <ExclamationTriangleIcon />
                  </Callout.Icon>
                  <Callout.Text>{sourceError}</Callout.Text>
                </Callout.Root>
              ) : mode === "pr" && boards.length === 0 ? (
                <Callout.Root color="orange" size="1">
                  <Callout.Icon>
                    <ExclamationTriangleIcon />
                  </Callout.Icon>
                  <Callout.Text>
                    No firmware builds found for this pull request. The CI
                    pipeline may not have finished yet, or this PR may not
                    produce firmware.
                  </Callout.Text>
                </Callout.Root>
              ) : (
                <Select.Root
                  value={selectedBoard}
                  onValueChange={handleBoardChange}
                  disabled={isConnected || boards.length === 0}
                >
                  <Select.Trigger
                    style={{ width: "100%" }}
                    placeholder="Select a board"
                  />
                  <Select.Content>
                    {boards.map((b) => (
                      <Select.Item key={b.value} value={b.value}>
                        {b.label}
                      </Select.Item>
                    ))}
                  </Select.Content>
                </Select.Root>
              )}
            </Flex>

            <Separator size="4" />

            <Flex gap="2">
              <Button
                onClick={handleConnect}
                disabled={!webSerialSupported || isFlashing}
                variant={isConnected ? "outline" : "solid"}
                style={{ flex: 1 }}
              >
                {flashState === "connecting"
                  ? "Connecting..."
                  : isConnected
                    ? "Disconnect"
                    : "Connect"}
              </Button>
              <Button
                onClick={handleFlash}
                disabled={!isConnected || isFlashing || !selectedBoard}
                variant="solid"
                color="green"
                style={{ flex: 1 }}
              >
                {isFlashing ? "Flashing..." : "Flash"}
              </Button>
              <AlertDialog.Root>
                <AlertDialog.Trigger>
                  <Button variant="soft" disabled={!selectedBoard}>
                    <DownloadIcon />
                  </Button>
                </AlertDialog.Trigger>
                <AlertDialog.Content maxWidth="400px">
                  <AlertDialog.Title>Download firmware</AlertDialog.Title>
                  <AlertDialog.Description>
                    You don't need to download the firmware to flash your
                    device. Just connect your board and hit Flash - it all
                    happens in the browser.
                  </AlertDialog.Description>
                  <Flex gap="3" mt="4" justify="end">
                    <AlertDialog.Cancel>
                      <Button variant="soft" color="gray">
                        Cancel
                      </Button>
                    </AlertDialog.Cancel>
                    <AlertDialog.Action>
                      <Button asChild variant="solid">
                        <a
                          href={downloadUrl ?? "#"}
                          download={`${selectedBoard}.bin`}
                        >
                          Download anyway
                        </a>
                      </Button>
                    </AlertDialog.Action>
                  </Flex>
                </AlertDialog.Content>
              </AlertDialog.Root>
            </Flex>

            {isFlashing && (
              <Flex direction="column" gap="2">
                <Flex justify="between">
                  <Text size="2" weight="medium">
                    Progress
                  </Text>
                  <Text size="2" color="gray">
                    {progress}%
                  </Text>
                </Flex>
                <Progress value={progress} />
              </Flex>
            )}

            {flashState === "done" && (
              <Callout.Root color="green" size="1">
                <Callout.Icon>
                  <InfoCircledIcon />
                </Callout.Icon>
                <Callout.Text>
                  Flash complete! Your device has been updated.
                </Callout.Text>
              </Callout.Root>
            )}
          </Flex>
        </Card>

        {logs.length > 0 && (
          <Card size="2" mt="4">
            <Text size="2" weight="medium" mb="2" as="p">
              Log
            </Text>
            <ScrollArea
              style={{
                height: 200,
                background: "var(--gray-2)",
                borderRadius: "var(--radius-2)",
              }}
              scrollbars="vertical"
            >
              <Code
                size="1"
                style={{ whiteSpace: "pre-wrap", display: "block" }}
              >
                <Box p="2">{logs.join("\n")}</Box>
              </Code>
              <div ref={logEndRef} />
            </ScrollArea>
          </Card>
        )}
      </Box>
    </Flex>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
