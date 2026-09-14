import type { Metadata } from 'next';
import './globals.css';
import './v2.css';

export const metadata: Metadata = {
  title: { default: 'Fileform — Local file tools', template: '%s — Fileform' },
  description: 'Free, open-source file tools. Convert, organize and transform files locally. The macOS app is working; the Windows desktop is in development.',
  alternates: { canonical: '/' },
  robots: { index: false, follow: false },
  metadataBase: new URL('https://fileform.amaangokak18.chatgpt.site'),
};
export default function RootLayout({children}:{children:React.ReactNode}) {
  return <html lang="en"><body>{children}</body></html>;
}
