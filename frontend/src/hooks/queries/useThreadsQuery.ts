import { useQuery } from "@tanstack/react-query";
import { listThreads } from "@/lib/api";

export const threadsQueryKey = (
  folderId: string,
  limit: number,
  offset: number,
  folderIds?: string[],
) => ["threads", folderId, folderIds, limit, offset] as const;

export function useThreadsQuery(
  folderId: string | null,
  limit = 50,
  offset = 0,
  folderIds?: string[],
) {
  return useQuery({
    queryKey: threadsQueryKey(folderId ?? "", limit, offset, folderIds),
    queryFn: () => listThreads(folderId!, limit, offset, folderIds),
    enabled: !!folderId,
    staleTime: 60_000,
  });
}
