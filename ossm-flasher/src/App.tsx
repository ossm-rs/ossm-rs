import { Terminal } from "@xterm/xterm";
import { ESPLoader, Transport } from "esptool-js";
import { createSignal, For, onMount, Show, type Component } from "solid-js";
import "../node_modules/@xterm/xterm/css/xterm.css";
import CryptoJS from "crypto-js";

const URL = "https://ossm.rs";
const BINARIES_URL = `${URL}/release_binaries`;
const BAUD = 921600;
const BOARDS = [
    "waveshare",
    "seeed_xiao_s3",
    "atom_s3",
    "ossm_v3",
    "custom_s3",
    "custom_c6",
    "ossm_alt_v2",
];

const App: Component = () => {
    const [board, setBoard] = createSignal(BOARDS[0]);
    const [connected, setConnected] = createSignal(false);
    const [flashing, setFlashing] = createSignal(false);

    let term: Terminal = new Terminal();
    let terminal_elem;
    let device: SerialPort | null = null;
    let transport: Transport | null = null;
    let espLoader: ESPLoader | null = null;

    const espLoaderTerminal = {
        clean() {
            term.clear();
        },
        writeLine(data) {
            term.writeln(data);
        },
        write(data) {
            term.write(data);
        },
    };

    function disconnect() {
        if (transport !== null) {
            transport.disconnect();
        }
        term.clear();
        setConnected(false);
    }

    async function onConnectButton(e) {
        if (connected()) {
            disconnect();
            return;
        }

        if (device === null) {
            device = await navigator.serial.requestPort();
        }

        transport = new Transport(device);

        espLoader = new ESPLoader({
            transport: transport,
            baudrate: BAUD,
            terminal: espLoaderTerminal,
        });

        const chip = await espLoader.main();

        setConnected(true);

        console.log(chip);
    }

    async function getBinary(target: string) {
        const response = await fetch(`${BINARIES_URL}/${target}.bin`);
        if (!response.ok) {
            throw new Error(`Response status: ${response.status}`);
        }

        return await response.bytes();
    }

    async function onFlashButton(e) {
        setFlashing(true);
        const binary = await getBinary(board());

        console.log(binary);

        if (espLoader === null) {
            alert("ESPLoader not loaded");
            return;
        }
        await espLoader.writeFlash({
            flashMode: "keep",
            flashFreq: "keep",
            flashSize: "keep",
            eraseAll: true,
            compress: true,
            fileArray: [{ address: 0, data: binary }],
            calculateMD5Hash: (image: Uint8Array) => {
                const latin1String = Array.from(image, (byte) =>
                    String.fromCharCode(byte),
                ).join("");
                return CryptoJS.MD5(
                    CryptoJS.enc.Latin1.parse(latin1String),
                ).toString();
            },
        });
        await espLoader.after();
        setFlashing(false);
    }

    onMount(() => {
        term.open(terminal_elem);
    });

    return (
        <div class="flex flex-col items-center min-h-screen min-w-screen bg-zinc-800">
            <p class="text-4xl text-orange-500 text-center pt-10 pb-10">
                Welcome to OSSM-RS Web Flasher
            </p>
            <p class="text-zinc-100">Select your board:</p>
            <select
                class="border-2 rounded-xl p-1 mb-6 w-lg text-2xl bg-zinc-100 disabled:bg-gray-400"
                value={board()}
                onChange={(e) => setBoard(e.currentTarget.value)}
                disabled={connected()}
            >
                <For each={BOARDS}>
                    {(item, index) => <option value={item}>{item}</option>}
                </For>
            </select>
            <button
                class="border-2 rounded-xl w-lg h-10 mb-2 text-2xl bg-zinc-100 disabled:bg-gray-400 hover:bg-orange-400 hover:cursor-pointer"
                onClick={onConnectButton}
                disabled={flashing()}
            >
                {connected() ? <p>Disconnect</p> : <p>Connect</p>}
            </button>
            <button
                class="border-2 rounded-xl w-lg h-10 text-2xl bg-zinc-100 disabled:bg-gray-400 hover:bg-orange-400 hover:cursor-pointer"
                onClick={onFlashButton}
                disabled={!connected() || flashing()}
            >
                Flash
            </button>
            <div class="flex flex-row mt-2 text-zinc-100">
                <p class="mr-2">State: </p>
                {connected() ? <p>Connected</p> : <p>Disconnected</p>}
            </div>
            <div class="mt-10" ref={terminal_elem}></div>
            <div class="grow"></div>
            <a class="my-4" href="https://github.com/orange-gem/ossm-rs/">
                <svg
                    width="50"
                    viewBox="0 0 98 96"
                    xmlns="http://www.w3.org/2000/svg"
                >
                    <path
                        fill-rule="evenodd"
                        clip-rule="evenodd"
                        d="M48.854 0C21.839 0 0 22 0 49.217c0 21.756 13.993 40.172 33.405 46.69 2.427.49 3.316-1.059 3.316-2.362 0-1.141-.08-5.052-.08-9.127-13.59 2.934-16.42-5.867-16.42-5.867-2.184-5.704-5.42-7.17-5.42-7.17-4.448-3.015.324-3.015.324-3.015 4.934.326 7.523 5.052 7.523 5.052 4.367 7.496 11.404 5.378 14.235 4.074.404-3.178 1.699-5.378 3.074-6.6-10.839-1.141-22.243-5.378-22.243-24.283 0-5.378 1.94-9.778 5.014-13.2-.485-1.222-2.184-6.275.486-13.038 0 0 4.125-1.304 13.426 5.052a46.97 46.97 0 0 1 12.214-1.63c4.125 0 8.33.571 12.213 1.63 9.302-6.356 13.427-5.052 13.427-5.052 2.67 6.763.97 11.816.485 13.038 3.155 3.422 5.015 7.822 5.015 13.2 0 18.905-11.404 23.06-22.324 24.283 1.78 1.548 3.316 4.481 3.316 9.126 0 6.6-.08 11.897-.08 13.526 0 1.304.89 2.853 3.316 2.364 19.412-6.52 33.405-24.935 33.405-46.691C97.707 22 75.788 0 48.854 0z"
                        fill="#fff"
                    />
                </svg>
            </a>
        </div>
    );
};

export default App;
