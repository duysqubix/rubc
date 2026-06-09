import type { Metadata, Viewport } from "next";
import { Pixelify_Sans, IBM_Plex_Mono, IBM_Plex_Sans } from "next/font/google";
import "./globals.css";
import { PwaRegister } from "@/components/PwaRegister";
import { EmulatorProvider } from "@/lib/store";

const pixelifySans = Pixelify_Sans({
  subsets: ["latin"],
  variable: "--font-pixel",
  weight: ["400", "500", "600", "700"],
});

const ibmPlexMono = IBM_Plex_Mono({
  subsets: ["latin"],
  variable: "--font-mono",
  weight: ["400", "500", "600", "700"],
});

const ibmPlexSans = IBM_Plex_Sans({
  subsets: ["latin"],
  variable: "--font-sans",
  weight: ["400", "500", "600", "700"],
});

export const metadata: Metadata = {
  title: "rubc Emulator",
  description: "GameBoy DMG/CGB Emulator",
  manifest: "/manifest.json",
  // iOS standalone install: these emit the apple-mobile-web-app-* meta tags so
  // "Add to Home Screen" launches rubc full-screen as an app (not a Safari tab).
  appleWebApp: {
    capable: true,
    title: "rubc",
    statusBarStyle: "black-translucent",
  },
  icons: {
    icon: "/icon-192.png",
    apple: "/icon-512.png",
  },
  // Next emits the standard mobile-web-app-capable but not the legacy Apple one,
  // which iOS still needs for true standalone mode + splash screens.
  other: {
    "apple-mobile-web-app-capable": "yes",
  },
};

export const viewport: Viewport = {
  themeColor: "#09090b",
  width: "device-width",
  initialScale: 1,
  maximumScale: 1,
  userScalable: false,
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className={`${pixelifySans.variable} ${ibmPlexMono.variable} ${ibmPlexSans.variable} antialiased`}>
        <EmulatorProvider>{children}</EmulatorProvider>
        <PwaRegister />
      </body>
    </html>
  );
}
