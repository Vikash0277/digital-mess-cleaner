import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { Sidebar, type NavGroup } from "./Sidebar";

const groups: NavGroup[] = [
  {
    id: "library",
    label: "Library",
    items: [
      { key: "/", label: "Dashboard" },
      { key: "/duplicates", label: "Duplicates" },
    ],
  },
  {
    id: "later",
    label: "Coming later",
    collapsible: true,
    items: [],
    subgroups: [
      {
        id: "release-2",
        label: "Release 2",
        items: [{ key: "/renaming", label: "Renaming" }],
      },
    ],
  },
];

function renderSidebar(initialPath = "/") {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <Sidebar groups={groups} />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  localStorage.clear();
  delete document.documentElement.dataset.sidebar;
});

describe("Sidebar", () => {
  it("lists library items and keeps later releases collapsed by default", () => {
    renderSidebar();

    expect(screen.getByRole("link", { name: "Dashboard" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Duplicates" })).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Renaming" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Coming later" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
  });

  it("expands the later group and remembers it", async () => {
    const user = userEvent.setup();
    renderSidebar();

    await user.click(screen.getByRole("button", { name: "Coming later" }));

    expect(screen.getByRole("link", { name: "Renaming" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Coming later" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(localStorage.getItem("dmc.nav.later")).toBe("1");
  });

  it("auto-expands the later group for a deep link", () => {
    renderSidebar("/renaming");

    expect(screen.getByRole("link", { name: "Renaming" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("marks the current route with aria-current and selects with the keyboard", async () => {
    const user = userEvent.setup();
    renderSidebar();

    expect(screen.getByRole("link", { name: "Dashboard" })).toHaveAttribute(
      "aria-current",
      "page",
    );

    await user.click(screen.getByRole("link", { name: "Duplicates" }));

    expect(screen.getByRole("link", { name: "Duplicates" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.getByRole("link", { name: "Dashboard" })).not.toHaveAttribute(
      "aria-current",
    );

    screen.getByRole("link", { name: "Dashboard" }).focus();
    await user.keyboard("{Enter}");

    expect(screen.getByRole("link", { name: "Dashboard" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("collapses to the rail and remembers the choice", async () => {
    const user = userEvent.setup();
    const first = renderSidebar();

    await user.click(screen.getByRole("button", { name: "Collapse sidebar" }));

    expect(document.documentElement.dataset.sidebar).toBe("collapsed");
    expect(localStorage.getItem("dmc.nav.collapsed")).toBe("1");
    expect(screen.getByRole("button", { name: "Expand sidebar" })).toBeInTheDocument();

    first.unmount();
    renderSidebar();

    await waitFor(() => {
      expect(document.documentElement.dataset.sidebar).toBe("collapsed");
    });
  });
});
