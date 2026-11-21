import { createFileRoute } from "@tanstack/react-router";
import { NotFound } from "../components/not-found";

// Every address no other route takes. The build prerenders it once, at /404, as 404.html.
export const Route = createFileRoute("/$")({ component: NotFound });
