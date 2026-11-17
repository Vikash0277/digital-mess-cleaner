import {
  createContext,
  useCallback,
  useContext,
  useState,
  type ReactNode,
} from "react";
import { ReviewDialog, type ReviewDetail } from "../components/ReviewDialog";

export interface ReviewRequest {
  title: string;
  message: string;
  details?: ReviewDetail[];
  note?: string;
  confirmPhrase?: string;
  primaryText?: string;
  secondaryText?: string;
  tone?: "primary" | "caution" | "danger";
}

export type ConfirmReview = (request: ReviewRequest) => Promise<boolean>;

const ReviewContext = createContext<ConfirmReview>(async () => false);

interface PendingReview {
  request: ReviewRequest;
  resolve: (confirmed: boolean) => void;
}

/**
 * App-wide approval gate. Every move, quarantine or delete asks here first,
 * shows item count and total size, and states whether the action is reversible.
 */
export function ReviewProvider({ children }: { children: ReactNode }) {
  const [pending, setPending] = useState<PendingReview | null>(null);

  const confirm = useCallback<ConfirmReview>(
    (request) =>
      new Promise<boolean>((resolve) => {
        setPending((existing) => {
          existing?.resolve(false);
          return { request, resolve };
        });
      }),
    [],
  );

  const settle = useCallback((confirmed: boolean) => {
    setPending((existing) => {
      existing?.resolve(confirmed);
      return null;
    });
  }, []);

  return (
    <ReviewContext.Provider value={confirm}>
      {children}
      <ReviewDialog
        open={pending !== null}
        title={pending?.request.title}
        message={pending?.request.message}
        details={pending?.request.details}
        note={pending?.request.note}
        confirmPhrase={pending?.request.confirmPhrase}
        primaryText={pending?.request.primaryText}
        secondaryText={pending?.request.secondaryText}
        tone={pending?.request.tone}
        onPrimary={() => settle(true)}
        onSecondary={() => settle(false)}
        onClose={() => settle(false)}
      />
    </ReviewContext.Provider>
  );
}

export function useReview(): ConfirmReview {
  return useContext(ReviewContext);
}
