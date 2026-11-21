import { createFileRoute } from "@tanstack/react-router";
import { PlatformPage } from "../components/platform-page";
import { platformHead } from "../seo";

export const Route = createFileRoute("/windows")({
  head: () => platformHead("windows"),
  component: () => <PlatformPage id="windows" />,
});
