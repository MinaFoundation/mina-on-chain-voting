import { siteConfig } from 'common/config';

export const Footer = () => {
  return (
    <footer className="py-6 md:px-8 md:py-0">
      <div className="container mx-auto items-center md:h-24 md:flex-row">
        {/* Main Footer Content */}
        <div className="flex flex-col lg:flex-row justify-between items-center lg:items-start space-y-4 lg:space-y-0">
          {/* Copyright Text */}
          <p className="text-gray-500 text-sm">© {new Date().getFullYear()} Mina Foundation. All rights reserved.</p>

          {/* Links */}
          <div className="flex flex-col lg:flex-row items-center space-y-2 lg:space-y-0 lg:space-x-6">
            <a
              href="https://github.com/MinaProtocol/mina/blob/develop/CODE_OF_CONDUCT.md"
              target="_blank"
              rel="noopener noreferrer"
              className="text-gray-500 text-sm dark:hover:text-white hover:text-gray-400"
            >
              Code of Conduct
            </a>
            <a
              href="https://minaprotocol.com/privacy"
              target="_blank"
              rel="noopener noreferrer"
              className="text-gray-500 text-sm dark:hover:text-white hover:text-gray-400"
            >
              Privacy Policy
            </a>
            <a
              href="https://minaprotocol.com/tos"
              target="_blank"
              rel="noopener noreferrer"
              className="text-gray-500 text-sm dark:hover:text-white hover:text-gray-400"
            >
              Terms of Service
            </a>
            <a
              href="https://minaprotocol.com/impressum"
              target="_blank"
              rel="noopener noreferrer"
              className="text-gray-500 text-sm dark:hover:text-white hover:text-gray-400"
            >
              Impressum
            </a>
          </div>
        </div>

        {/* Built by Granola */}
        <div className="mt-4 text-gray-500 text-sm text-center lg:text-left">
          Built by{' '}
          <a
            href={siteConfig.links.granola}
            target="_blank"
            rel="noreferrer"
            className="font-medium underline underline-offset-4 text-logoOrange"
          >
            Granola
          </a>
        </div>
      </div>
    </footer>
  );
};
