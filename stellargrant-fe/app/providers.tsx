"use client";

/**
 * Providers Component
 *
 * Client-side provider wrapper for QueryClient, StellarGrantsProvider,
 * and SocketProvider. This component is imported into the root layout.
 */

import { QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { queryClient } from "@/lib/queryClient";
import { StellarGrantsProvider } from "@/components/StellarGrantsProvider";
import { SocketProvider } from "@/hooks/useSocket";
import { NotificationToast } from "@/components/ui/NotificationToast";

import { OfflineBanner } from "@/components/layout/OfflineBanner";
import { useNetworkStatus } from "@/hooks/useNetworkStatus";
import { useEffect } from "react";

export function Providers({ children }: { children: React.ReactNode }) {
  const { isOnline } = useNetworkStatus();

  useEffect(() => {
    if (isOnline) {
      void queryClient.refetchQueries({ type: "active" });
    }
  }, [isOnline]);

  return (
    <QueryClientProvider client={queryClient}>
      <StellarGrantsProvider>
        <SocketProvider>
          <OfflineBanner />
          {children}
          <NotificationToast />
        </SocketProvider>
      </StellarGrantsProvider>
      {process.env.NODE_ENV === "development" && (
        <ReactQueryDevtools initialIsOpen={false} />
      )}
    </QueryClientProvider>
  );
}
