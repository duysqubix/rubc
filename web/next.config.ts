import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "export",
  // Allow the dev HMR websocket from the LAN/Tailscale host so on-device testing
  // against `next dev` doesn't get its handshake rejected (which stalls React 19
  // hydration -> dead UI). No effect on the static export / production.
  allowedDevOrigins: ["macbook-pro", "macbook-pro.moray-gila.ts.net", "100.115.95.66"],
};

export default nextConfig;
