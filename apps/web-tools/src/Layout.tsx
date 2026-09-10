import { useState } from "react";
import { NavLink, Outlet } from "react-router";
import { useAppearance } from "./hooks/useAppearance";
import { useIsMobile } from "./hooks/useIsMobile";
import { Drawer } from "vaul";
import {
  Theme,
  Flex,
  TabNav,
  IconButton,
  Button,
  Tooltip,
  Text,
  Separator,
} from "@radix-ui/themes";
import {
  SunIcon,
  MoonIcon,
  GitHubLogoIcon,
  HamburgerMenuIcon,
  CubeIcon,
  BarChartIcon,
  LayersIcon,
  RocketIcon,
} from "@radix-ui/react-icons";

export default function Layout() {
  const [appearance, toggleAppearance] = useAppearance();
  const isMobile = useIsMobile();
  const [menuOpen, setMenuOpen] = useState(false);

  return (
    <Theme asChild accentColor="purple" radius="large" appearance={appearance}>
      <Flex direction="column" style={{ height: "100%" }}>
        {isMobile ? (
          <Flex
            align="center"
            justify="between"
            px="3"
            py="2"
            style={{ borderBottom: "1px solid var(--gray-5)" }}
          >
            <Text size="3" weight="bold">
              ossm-rs
              <Text as="span" color="gray" ml="1" style={{ verticalAlign: "super", fontSize: "0.6em", fontWeight: "bold" }}>
                BETA
              </Text>
            </Text>
            <Drawer.Root direction="right" open={menuOpen} onOpenChange={setMenuOpen}>
              <Drawer.Trigger asChild>
                <IconButton variant="ghost" size="2">
                  <HamburgerMenuIcon width={20} height={20} />
                </IconButton>
              </Drawer.Trigger>
              <Drawer.Portal>
                <Drawer.Overlay
                  style={{
                    position: "fixed",
                    inset: 0,
                    backgroundColor: "rgba(0,0,0,0.4)",
                    zIndex: 50,
                  }}
                />
                <Drawer.Content
                  style={{
                    position: "fixed",
                    top: 0,
                    right: 0,
                    bottom: 0,
                    width: 260,
                    zIndex: 51,
                    outline: "none",
                  }}
                >
                  <Theme accentColor="purple" radius="large" appearance={appearance} style={{ height: "100%", borderLeft: "1px solid var(--gray-5)" }}>
                  <Flex direction="column" gap="1" p="4" style={{ height: "100%" }}>
                    <Drawer.Title asChild>
                      <Text size="4" weight="bold" mb="3">
                        ossm-rs
                        <Text as="span" color="gray" ml="1" style={{ verticalAlign: "super", fontSize: "0.6em", fontWeight: "bold" }}>
                          BETA
                        </Text>
                      </Text>
                    </Drawer.Title>

                    <NavLink to="/simulator" end onClick={() => setMenuOpen(false)} style={{ textDecoration: "none" }}>
                      {({ isActive }) => (
                        <Flex align="center" gap="2" px="2" py="2" style={{ borderRadius: 6, backgroundColor: isActive ? "var(--accent-3)" : "transparent" }}>
                          <CubeIcon />
                          <Text size="2" weight={isActive ? "medium" : "regular"}>Simulator</Text>
                        </Flex>
                      )}
                    </NavLink>
                    <NavLink to="/graph" onClick={() => setMenuOpen(false)} style={{ textDecoration: "none" }}>
                      {({ isActive }) => (
                        <Flex align="center" gap="2" px="2" py="2" style={{ borderRadius: 6, backgroundColor: isActive ? "var(--accent-3)" : "transparent" }}>
                          <BarChartIcon />
                          <Text size="2" weight={isActive ? "medium" : "regular"}>Graph</Text>
                        </Flex>
                      )}
                    </NavLink>
                    <NavLink to="/diagram" onClick={() => setMenuOpen(false)} style={{ textDecoration: "none" }}>
                      {({ isActive }) => (
                        <Flex align="center" gap="2" px="2" py="2" style={{ borderRadius: 6, backgroundColor: isActive ? "var(--accent-3)" : "transparent" }}>
                          <LayersIcon />
                          <Text size="2" weight={isActive ? "medium" : "regular"}>Diagram</Text>
                        </Flex>
                      )}
                    </NavLink>
                    <NavLink to="/firmware" onClick={() => setMenuOpen(false)} style={{ textDecoration: "none" }}>
                      {({ isActive }) => (
                        <Flex align="center" gap="2" px="2" py="2" style={{ borderRadius: 6, backgroundColor: isActive ? "var(--accent-3)" : "transparent" }}>
                          <RocketIcon />
                          <Text size="2" weight={isActive ? "medium" : "regular"}>Firmware</Text>
                        </Flex>
                      )}
                    </NavLink>

                    <Separator size="4" my="2" />

                    <Flex align="center" gap="2" px="2" py="2" onClick={() => { toggleAppearance(); setMenuOpen(false); }} style={{ borderRadius: 6, cursor: "pointer" }}>
                      {appearance === "light" ? <SunIcon /> : <MoonIcon />}
                      <Text size="2">{appearance === "light" ? "Dark mode" : "Light mode"}</Text>
                    </Flex>
                    <a
                      href="https://github.com/ossm-rs/ossm"
                      target="_blank"
                      rel="noopener noreferrer"
                      style={{ textDecoration: "none" }}
                    >
                      <Flex align="center" gap="2" px="2" py="2" style={{ borderRadius: 6 }}>
                        <GitHubLogoIcon />
                        <Text size="2">GitHub</Text>
                      </Flex>
                    </a>
                  </Flex>
                  </Theme>
                </Drawer.Content>
              </Drawer.Portal>
            </Drawer.Root>
          </Flex>
        ) : (
          <Flex
            align="center"
            justify="between"
            px="4"
            style={{
              borderBottom: "1px solid var(--gray-5)",
              overflow: "hidden",
            }}
          >
            <Flex align="center" gap="4">
              <Text size="3" weight="bold">
                ossm-rs
                <Text as="span" color="gray" ml="1" style={{ verticalAlign: "super", fontSize: "0.6em", fontWeight: "bold" }}>
                  BETA
                </Text>
              </Text>
              <TabNav.Root>
                <NavLink to="/simulator" end>
                  {({ isActive }) => (
                    <TabNav.Link asChild active={isActive}>
                      <span>Simulator</span>
                    </TabNav.Link>
                  )}
                </NavLink>
                <NavLink to="/graph">
                  {({ isActive }) => (
                    <TabNav.Link asChild active={isActive}>
                      <span>Graph</span>
                    </TabNav.Link>
                  )}
                </NavLink>
                <NavLink to="/diagram">
                  {({ isActive }) => (
                    <TabNav.Link asChild active={isActive}>
                      <span>Diagram</span>
                    </TabNav.Link>
                  )}
                </NavLink>
                <NavLink to="/firmware">
                  {({ isActive }) => (
                    <TabNav.Link asChild active={isActive}>
                      <span>Firmware</span>
                    </TabNav.Link>
                  )}
                </NavLink>
              </TabNav.Root>
            </Flex>
            <Flex align="center" gap="2">
              <Button variant="soft" size="2" asChild>
                <a
                  href="https://github.com/ossm-rs/ossm"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  <GitHubLogoIcon /> ossm-rs/ossm
                </a>
              </Button>
              <Tooltip
                content={
                  appearance === "light"
                    ? "Switch to dark mode"
                    : "Switch to light mode"
                }
              >
                <IconButton
                  variant="soft"
                  size="2"
                  onClick={toggleAppearance}
                  aria-label="Toggle theme"
                >
                  {appearance === "light" ? <SunIcon /> : <MoonIcon />}
                </IconButton>
              </Tooltip>
            </Flex>
          </Flex>
        )}

        <Flex style={{ flex: 1, minHeight: 0 }}>
          <Outlet />
        </Flex>
      </Flex>
    </Theme>
  );
}
