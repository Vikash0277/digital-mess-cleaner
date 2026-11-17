import { render, screen } from "@testing-library/react";
import { EmptyState } from "./EmptyState";

describe("EmptyState", () => {
  it("shows the title and description", () => {
    render(<EmptyState title="No files" description="Add a folder to start scanning." />);

    expect(screen.getByRole("heading", { name: "No files" })).toBeInTheDocument();
    expect(screen.getByText("Add a folder to start scanning.")).toBeInTheDocument();
  });

  it("does not render a string icon name as text", () => {
    render(
      <EmptyState title="No drives" description="Choose a drive." icon="hard-drive" />,
    );

    expect(screen.queryByText("hard-drive")).not.toBeInTheDocument();
  });

  it("renders a node icon and the optional action", () => {
    render(
      <EmptyState
        title="Nothing here"
        description="Try again."
        icon={<svg data-testid="empty-icon" />}
        buttonText="Add location"
      />,
    );

    expect(screen.getByTestId("empty-icon")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add location" })).toBeInTheDocument();
  });
});
