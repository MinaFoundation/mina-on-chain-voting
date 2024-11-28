'use client';

import * as React from 'react';

import Link, { LinkProps } from 'next/link';
import { useRouter } from 'next/navigation';

import { siteConfig } from 'common/config';
import { cn } from 'common/utils';

import { Button } from 'components/core/button';
import { ScrollArea } from 'components/core/scroll-area';
import { Sheet, SheetContent, SheetTrigger } from 'components/core/sheet';

import { ViewVerticalIcon } from '@radix-ui/react-icons';
import { MinaLogo } from './mina-logo';

export const NavigationMobile = () => {
  const [open, setOpen] = React.useState(false);

  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetTrigger asChild>
        <Button
          variant="ghost"
          className="mr-2 px-0 text-base hover:bg-transparent focus-visible:bg-transparent focus-visible:ring-0 focus-visible:ring-offset-0 md:hidden"
        >
          <ViewVerticalIcon className="h-5 w-5" />
          <span className="sr-only">Toggle Menu</span>
        </Button>
      </SheetTrigger>
      <SheetContent side="left" className="pr-0">
        <MinaLogo />
        <ScrollArea className="my-4 h-[calc(100vh-8rem)] pb-10 pl-6">
          <div className="flex flex-col space-y-3">
            {siteConfig.nav?.map((item) => (
              <MobileLink key={item.href} href={item.href} onOpenChange={setOpen}>
                {item.title}
              </MobileLink>
            ))}
          </div>
        </ScrollArea>
      </SheetContent>
    </Sheet>
  );
};

interface MobileLinkProps extends LinkProps {
  onOpenChange?: (open: boolean) => void;
  children: React.ReactNode;
  className?: string;
}

const MobileLink = ({ href, onOpenChange, className, children, ...props }: MobileLinkProps) => {
  const router = useRouter();

  return (
    <Link
      href={href}
      onClick={() => {
        router.push(href.toString());
        onOpenChange?.(false);
      }}
      className={cn(className)}
      {...props}
    >
      {children}
    </Link>
  );
};
