import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { ReviewProvider, useReview, type ReviewRequest } from "./review";

function Harness({ request, onResult }: { request: ReviewRequest; onResult: (value: boolean) => void }) {
  const confirm = useReview();
  return (
    <button
      type="button"
      onClick={() => {
        void confirm(request).then(onResult);
      }}
    >
      Ask for review
    </button>
  );
}

const quarantineRequest: ReviewRequest = {
  title: "Move 3 files to quarantine?",
  message: "They can be restored from the Cleanup Centre.",
  details: [
    { label: "Files", value: "3" },
    { label: "Total size", value: "128.0 MB" },
  ],
  note: "Reversible. Nothing is permanently deleted in this step.",
  primaryText: "Move to quarantine",
  secondaryText: "Keep files",
  tone: "caution",
};

describe("ReviewProvider", () => {
  it("blocks the action until the user confirms", async () => {
    const user = userEvent.setup();
    const onResult = vi.fn();

    render(
      <ReviewProvider>
        <Harness request={quarantineRequest} onResult={onResult} />
      </ReviewProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Ask for review" }));

    const dialog = screen.getByRole("dialog", { name: "Move 3 files to quarantine?" });
    expect(dialog).toBeInTheDocument();
    expect(screen.getByText("128.0 MB")).toBeInTheDocument();
    expect(screen.getByText(/Reversible/)).toBeInTheDocument();
    expect(onResult).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Move to quarantine" }));

    expect(onResult).toHaveBeenCalledWith(true);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("resolves false when the user keeps the files", async () => {
    const user = userEvent.setup();
    const onResult = vi.fn();

    render(
      <ReviewProvider>
        <Harness request={quarantineRequest} onResult={onResult} />
      </ReviewProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Ask for review" }));
    await user.click(screen.getByRole("button", { name: "Keep files" }));

    expect(onResult).toHaveBeenCalledWith(false);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("keeps permanent delete disabled until the exact phrase is typed", async () => {
    const user = userEvent.setup();
    const onResult = vi.fn();

    render(
      <ReviewProvider>
        <Harness
          request={{
            title: "Permanently delete 2 files?",
            message: "This cannot be undone.",
            confirmPhrase: "DELETE PERMANENTLY",
            primaryText: "Delete permanently",
            tone: "danger",
          }}
          onResult={onResult}
        />
      </ReviewProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Ask for review" }));

    const primary = screen.getByRole("button", { name: "Delete permanently" });
    expect(primary).toBeDisabled();
    expect(primary).toHaveClass("btn-danger");

    await user.type(screen.getByLabelText(/Type/i), "delete permanently");
    expect(primary).toBeDisabled();

    await user.clear(screen.getByLabelText(/Type/i));
    await user.type(screen.getByLabelText(/Type/i), "DELETE PERMANENTLY");
    expect(primary).toBeEnabled();

    await user.click(primary);
    expect(onResult).toHaveBeenCalledWith(true);
  });
});

describe("ReviewProvider dialog lifecycle", () => {
  it("only shows one dialog when reviews overlap", async () => {
    const user = userEvent.setup();
    const results: boolean[] = [];

    function Overlap() {
      const confirm = useReview();
      const [second, setSecond] = useState(false);
      return (
        <button
          type="button"
          onClick={() => {
            if (!second) {
              void confirm(quarantineRequest).then((value) => results.push(value));
              setSecond(true);
              void confirm({ ...quarantineRequest, title: "Second review" }).then((value) =>
                results.push(value),
              );
            }
          }}
        >
          Ask twice
        </button>
      );
    }

    render(
      <ReviewProvider>
        <Overlap />
      </ReviewProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Ask twice" }));

    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    expect(screen.getByRole("dialog", { name: "Second review" })).toBeInTheDocument();
    expect(results).toEqual([false]);
  });
});
