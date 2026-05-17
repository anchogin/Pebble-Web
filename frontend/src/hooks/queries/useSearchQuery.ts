import { useQuery } from "@tanstack/react-query";
import { searchMessages } from "@/lib/api";

export const searchQueryKey = (query: string, limit?: number) =>
  ["search", query, limit] as const;

export function useSearchQuery(query: string, limit?: number) {
  return useQuery({
    queryKey: searchQueryKey(query, limit),
    queryFn: () => searchMessages(query, limit),
    enabled: query.length > 0,
  });
}
