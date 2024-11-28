import { PropsWithChildren } from 'react';

import { Metadata } from 'next';
import { IBM_Plex_Sans } from 'next/font/google';

import { siteConfig } from 'common/config';
import { cn } from 'common/utils';

import { Toaster } from 'components/core/toaster';
import { Footer } from 'components/layout-footer';
import { Header } from 'components/layout-header';
import { Providers } from 'components/providers';

import './globals.css';

const FONT = IBM_Plex_Sans({ weight: ['400'], subsets: ['latin'] });

export const metadata: Metadata = {
  title: `Mina - ${siteConfig.title}`,
  themeColor: [
    { media: '(prefers-color-scheme: light)', color: 'white' },
    { media: '(prefers-color-scheme: dark)', color: 'black' },
  ],
};

const Layout = ({ children }: PropsWithChildren) => {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className={cn('bg-background antialiased', FONT.className)}>
        <Providers>
          <div className="relative flex flex-col min-h-screen">
            <Header />
            <div className="flex-1">{children}</div>
            <Footer />
          </div>
          <Toaster />
        </Providers>
      </body>
    </html>
  );
};

export default Layout;
