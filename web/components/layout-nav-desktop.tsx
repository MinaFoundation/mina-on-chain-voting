'use client';

import * as React from 'react';

import Link from 'next/link';

import { MinaLogo } from './mina-logo';

export const NavigationDesktop = () => {
  return (
    <div className="mr-4 hidden md:flex">
      <Link href="/" className="mr-6 flex items-center space-x-2">
        <MinaLogo />
      </Link>
    </div>
  );
};
