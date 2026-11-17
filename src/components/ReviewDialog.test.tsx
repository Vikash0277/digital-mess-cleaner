import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ReviewDialog } from "./ReviewDialog";

describe("ReviewDialog", () => {
  it("shows the preview and uses the caution tone for review-first actions", () => {
    render(
      <ReviewDialog
        open
        title="Quarantine 3 files"
        message="They can be restored from the Cleanup Centre."
        primaryText="Move to quarantine"
        secondaryText="Cancel"
        tone="caution"
      />,
    );

    expect(screen.getByRole("dialog", { name: "Quarantine 3 files" })).toBeInTheDocument();
    expect(
      screen.getByText("They can be restored from the Cleanup Centre."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Move to quarantine" })).toHaveClass(
      "btn-caution",
    );
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<ReviewDialog open title="Review" message="Check the files." onClose={onClose} />);

    await user.keyboard("{Escape}");

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("cancels and closes from the secondary action", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const onSecondary = vi.fn();
    render(
      <ReviewDialog
        open
        title="Review"
        message="Check the files."
        secondaryText="Keep files"
        onClose={onClose}
        onSecondary={onSecondary}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Keep files" }));

    expect(onSecondary).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("uses the danger tone for permanent deletion", () => {
    render(
      <ReviewDialog
        open
        title="Delete permanently"
        message="This cannot be undone."
        primaryText="Delete permanently"
        destructive
      />,
    );

    expect(
      screen.getByRole("button", { name: "Delete permanently" }),
    ).toHaveClass("btn-danger");
  });
});
