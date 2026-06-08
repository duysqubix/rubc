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
