import { createFileRoute } from "@tanstack/react-router";
import { PlatformPage } from "../components/platform-page";
import { platformHead } from "../seo";

export const Route = createFileRoute("/mac")({
  head: () => platformHead("mac"),
  component: () => <PlatformPage id="mac" />,
});
