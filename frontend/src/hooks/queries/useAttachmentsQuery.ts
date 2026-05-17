import { useQuery } from "@tanstack/react-query";
import { listAttachments } from "@/lib/api";

export const attachmentsQueryKey = (messageId: string) =>
  ["attachments", messageId] as const;

export function useAttachmentsQuery(messageId: string | null) {
  return useQuery({
    queryKey: attachmentsQueryKey(messageId ?? ""),
    queryFn: () => listAttachments(messageId!),
    enabled: !!messageId,
  });
}
