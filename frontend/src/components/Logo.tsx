import React from "react";
import Image from "next/image";
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "./ui/dialog";
import { VisuallyHidden } from "./ui/visually-hidden";
import { About } from "./About";
import { APP_NAME, APP_TAG } from "@/lib/appName";

interface LogoProps {
  isCollapsed: boolean;
}

const Logo = React.forwardRef<HTMLButtonElement, LogoProps>(
  ({ isCollapsed }, ref) => {
    return (
      <Dialog aria-describedby={undefined}>
        {isCollapsed ? (
          <DialogTrigger asChild>
            <button
              ref={ref}
              type="button"
              className="flex items-center justify-center mb-2 cursor-pointer bg-transparent border-none p-0 hover:opacity-80 transition-opacity"
              aria-label={`About ${APP_NAME} ${APP_TAG}`}
            >
              <Image
                src="/logo-collapsed.png"
                alt={APP_NAME}
                width={40}
                height={40}
                className="object-contain"
                priority
              />
            </button>
          </DialogTrigger>
        ) : (
          <DialogTrigger asChild>
            <button
              ref={ref}
              type="button"
              className="w-full text-left px-1 mb-2 flex items-center gap-2 cursor-pointer hover:opacity-80 transition-opacity focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              aria-label={`About ${APP_NAME} ${APP_TAG}`}
            >
              <Image
                src="/logo-collapsed.png"
                alt=""
                width={24}
                height={24}
                className="object-contain"
                priority
              />
              <span className="font-heading text-xl font-bold tracking-tight text-foreground">{APP_NAME}</span>
              <span className="rounded-full bg-blue-50 px-2 py-0.5 text-[11px] font-semibold uppercase tracking-wide text-blue-700 ring-1 ring-inset ring-blue-200">
                {APP_TAG}
              </span>
            </button>
          </DialogTrigger>
        )}
        <DialogContent>
          <VisuallyHidden>
            <DialogTitle>About {APP_NAME} {APP_TAG}</DialogTitle>
          </VisuallyHidden>
          <About />
        </DialogContent>
      </Dialog>
    );
  },
);

Logo.displayName = "Logo";

export default Logo;
