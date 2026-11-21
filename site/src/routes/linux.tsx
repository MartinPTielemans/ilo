import { createFileRoute } from "@tanstack/react-router";
import { PlatformPage } from "../components/platform-page";
import { platformHead } from "../seo";

export const Route = createFileRoute("/linux")({
  head: () => platformHead("linux"),
  component: () => <PlatformPage id="linux" />,
});
