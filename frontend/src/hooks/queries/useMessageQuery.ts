import { useQuery } from "@tanstack/react-query";
import { getMessage } from "@/lib/api";

export const messageQueryKey = (messageId: string) =>
  ["message", messageId] as const;

export function useMessageQuery(messageId: string | null) {
  return useQuery({
    queryKey: messageQueryKey(messageId ?? ""),
    queryFn: () => getMessage(messageId!),
    enabled: !!messageId,
    staleTime: 5 * 60_000,
  });
}
