import { useQuery } from "@tanstack/react-query";
import { getPendingMailOpsSummary } from "@/lib/api";

export const pendingMailOpsSummaryQueryKey = (accountId: string | null) =>
  ["pendingMailOps", accountId] as const;

export function usePendingMailOpsSummary(accountId: string | null) {
  return useQuery({
    queryKey: pendingMailOpsSummaryQueryKey(accountId),
    queryFn: () => getPendingMailOpsSummary(accountId),
    refetchInterval: 15_000,
  });
}
