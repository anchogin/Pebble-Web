import { useQuery } from "@tanstack/react-query";
import { listThreadMessages } from "@/lib/api";

export const threadMessagesQueryKey = (threadId: string) =>
  ["threadMessages", threadId] as const;

export function useThreadMessagesQuery(threadId: string | null) {
  return useQuery({
    queryKey: threadMessagesQueryKey(threadId ?? ""),
    queryFn: () => listThreadMessages(threadId!),
    enabled: !!threadId,
  });
}
