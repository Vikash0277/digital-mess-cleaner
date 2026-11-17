import { render, screen } from "@testing-library/react";
import { ProgressBar } from "./ProgressBar";

describe("ProgressBar", () => {
  it("exposes its value and label", () => {
    render(<ProgressBar progress={62} label="62% checked" />);

    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "62");
    expect(screen.getByText("62% checked")).toBeInTheDocument();
  });

  it("clamps values to the valid range", () => {
    render(<ProgressBar progress={140} showValue />);

    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "100");
    expect(screen.getByText("100%")).toBeInTheDocument();
  });
});
