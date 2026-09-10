import type { Metadata } from "next";
import "./globals.css";
import { QueryProvider } from "@/providers/query-provider";
import { GenomeProvider } from "@/providers/genome-provider";
import { ThemeProvider } from "@/providers/theme-provider";
import { CoordinateProvider } from "@/providers/coordinate-provider";

export const metadata: Metadata = {
  title: "Genome Guides",
  description: "An interactive catalog of the human genome.",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="antialiased font-sans selection:bg-primary/10 selection:text-primary">
        <QueryProvider>
          <GenomeProvider>
            <CoordinateProvider>
              <ThemeProvider
                attribute="class"
                defaultTheme="system"
                enableSystem
                disableTransitionOnChange
              >
                {children}
              </ThemeProvider>
            </CoordinateProvider>
          </GenomeProvider>
        </QueryProvider>
      </body>
    </html>
  );
}
