import Link from 'next/link';

import { siteConfig } from 'common/config';
import { cn } from 'common/utils';

import { buttonVariants } from 'components/core/button';
import { Icons } from 'components/core/icons';
import { NavigationDesktop } from 'components/layout-nav-desktop';
import { NavigationMobile } from 'components/layout-nav-mobile';
import { ThemeToggle } from 'components/theme-toggle';

export const Header = () => {
  return (
    <header
      className="sticky top-0 z-50 w-full bg-header-gradient backdrop-blur"
      style={{
        background:
          'linear-gradient(90deg, rgba(49,130,206,1) 0%, rgba(130,113,194,1) 50%, rgba(175,120,219,1) 75%, rgba(141,188,235,1) 100%)',
        padding: '40px 0',
      }}
    >
      <div className="container flex h-14 items-center">
        <NavigationDesktop />
        <NavigationMobile />
        <div className="flex flex-1 items-center justify-end space-x-4">
          <nav className="flex items-center space-x-1">
            <Link href={siteConfig.links.FAQ} target="_blank">
              <div
                className={cn(
                  buttonVariants({
                    variant: 'ghost',
                  }),
                  'w-9 px-0',
                )}
              >
                <span className="text-white hover:text-foreground/80">FAQ</span>
              </div>
            </Link>
            <Link href={siteConfig.links.github} target="_blank">
              <div
                className={cn(
                  buttonVariants({
                    variant: 'ghost',
                  }),
                  'w-9 px-0',
                )}
              >
                <Icons.gitHub className="h-4 w-4 text-white hover:text-foreground/80" />
                <span className="sr-only">GitHub</span>
              </div>
            </Link>
            <ThemeToggle />
          </nav>
        </div>
      </div>
    </header>
  );
};
