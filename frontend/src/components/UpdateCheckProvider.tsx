'use client'

import React, { createContext, useContext } from 'react';
import { UpdateInfo } from '@/services/updateService';

// Automatic updates are disabled in this build: no update checks are made.

interface UpdateCheckContextType {
  updateInfo: UpdateInfo | null;
  isChecking: boolean;
  checkForUpdates: (force?: boolean) => Promise<void>;
  showUpdateDialog: () => void;
}

const UpdateCheckContext = createContext<UpdateCheckContextType | undefined>(undefined);

const DISABLED: UpdateCheckContextType = {
  updateInfo: null,
  isChecking: false,
  checkForUpdates: async () => {},
  showUpdateDialog: () => {},
};

export function UpdateCheckProvider({ children }: { children: React.ReactNode }) {
  return <UpdateCheckContext.Provider value={DISABLED}>{children}</UpdateCheckContext.Provider>;
}

export function useUpdateCheckContext() {
  const context = useContext(UpdateCheckContext);
  if (context === undefined) {
    throw new Error('useUpdateCheckContext must be used within UpdateCheckProvider');
  }
  return context;
}
