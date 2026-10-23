'use client';

import React from 'react';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';

interface MainContentProps {
  children: React.ReactNode;
  /** No sidebar is shown: use the whole window width */
  fullWidth?: boolean;
}

const MainContent: React.FC<MainContentProps> = ({ children, fullWidth = false }) => {
  const { isCollapsed } = useSidebar();
  const sidebarMargin = fullWidth ? '' : isCollapsed ? 'ml-16' : 'ml-64';

  return (
    <main
      className={`flex-1 min-w-0 overflow-hidden transition-all duration-300 ${sidebarMargin}`}
    >
      <div className={`${fullWidth ? '' : 'pl-8 '}min-w-0 w-full max-w-full overflow-hidden`}>
        {children}
      </div>
    </main>
  );
};

export default MainContent;
