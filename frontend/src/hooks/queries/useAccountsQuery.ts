import { useQuery } from "@tanstack/react-query";
import { listAccounts } from "@/lib/api";

export const accountsQueryKey = ["accounts"] as const;

export function useAccountsQuery() {
  return useQuery({
    queryKey: accountsQueryKey,
    queryFn: listAccounts,
  });
}
