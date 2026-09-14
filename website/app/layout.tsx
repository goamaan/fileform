import type { Metadata } from 'next';
import './globals.css';
import './v2.css';

export const metadata: Metadata = {
  title: { default: 'Fileform — File tools for Mac', template: '%s — Fileform' },
  description: 'Convert images, PDFs, audio, video and tables locally on your Mac. Fileform is in development; its open-source CLI is available now.',
  alternates: { canonical: '/' },
  robots: { index: false, follow: false },
  metadataBase: new URL('https://fileform.amaangokak18.chatgpt.site'),
};
export default function RootLayout({children}:{children:React.ReactNode}) {
  return <html lang="en"><body>{children}</body></html>;
}
