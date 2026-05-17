import { useQuery } from "@tanstack/react-query";
import { listPendingMailOps } from "@/lib/api";

export const pendingMailOpsQueryKey = (accountId: string | null) =>
  ["pendingMailOpsList", accountId] as const;

export function usePendingMailOpsQuery(accountId: string | null, limit = 100) {
  return useQuery({
    queryKey: pendingMailOpsQueryKey(accountId),
    queryFn: () => listPendingMailOps(accountId, limit),
    refetchInterval: 15_000,
  });
}
