import { ChevronRight, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { useEffect, useLayoutEffect, useState } from "react";
import { NavLink, useLocation } from "react-router-dom";

export type NavItem = {
  key: string;
  label: string;
  Icon?: React.ReactNode;
  badge?: string;
};

export type NavGroup = {
  id: string;
  label: string;
  items: NavItem[];
  subgroups?: NavGroup[];
  collapsible?: boolean;
};

const COLLAPSED_KEY = "dmc.nav.collapsed";
const LATER_KEY = "dmc.nav.later";
const NARROW_QUERY = "(max-width: 720px)";

function readFlag(key: string): boolean {
  try {
    return localStorage.getItem(key) === "1";
  } catch {
    return false;
  }
}

function writeFlag(key: string, value: boolean) {
  try {
    localStorage.setItem(key, value ? "1" : "0");
  } catch {
    /* Storage can be unavailable; the sidebar still works for this session. */
  }
}

function isActive(key: string, pathname: string): boolean {
  if (key === "/") return pathname === "/";
  return pathname === key || pathname.startsWith(`${key}/`);
}

function groupHasActive(group: NavGroup, pathname: string): boolean {
  return (
    group.items.some((item) => isActive(item.key, pathname)) ||
    (group.subgroups ?? []).some((subgroup) => groupHasActive(subgroup, pathname))
  );
}

function GroupItems({ group }: { group: NavGroup }) {
  if (group.items.length === 0) return null;

  return (
    <div className="sidebar-section-items">
      {group.items.map((item) => (
        <NavLink
          key={item.key}
          to={item.key}
          end={item.key === "/"}
          className="sidebar-item"
          title={item.label}
          data-label={item.label}
        >
          <span className="sidebar-item-icon" aria-hidden="true">
            {item.Icon}
          </span>
          <span className="sidebar-item-label">{item.label}</span>
          {item.badge && <span className="nav-badge">{item.badge}</span>}
        </NavLink>
      ))}
    </div>
  );
}

export function Sidebar({ groups }: { groups: NavGroup[] }) {
  const { pathname } = useLocation();
  const laterGroup = groups.find((group) => group.collapsible);
  const laterActive = laterGroup ? groupHasActive(laterGroup, pathname) : false;

  const [manualCollapsed, setManualCollapsed] = useState(() => readFlag(COLLAPSED_KEY));
  const [narrow, setNarrow] = useState(() => {
    try {
      return window.matchMedia(NARROW_QUERY).matches;
    } catch {
      return false;
    }
  });
  const [laterOpen, setLaterOpen] = useState(() => laterActive || readFlag(LATER_KEY));
  const collapsed = manualCollapsed || narrow;

  useEffect(() => {
    const query = window.matchMedia(NARROW_QUERY);
    function handleChange(event: MediaQueryListEvent) {
      setNarrow(event.matches);
    }
    query.addEventListener("change", handleChange);
    return () => query.removeEventListener("change", handleChange);
  }, []);

  useLayoutEffect(() => {
    document.documentElement.dataset.sidebar = collapsed ? "collapsed" : "expanded";
    return () => {
      delete document.documentElement.dataset.sidebar;
    };
  }, [collapsed]);

  useEffect(() => {
    if (laterActive) setLaterOpen(true);
  }, [laterActive]);

  function toggleCollapsed() {
    const next = !manualCollapsed;
    setManualCollapsed(next);
    writeFlag(COLLAPSED_KEY, next);
  }

  function toggleLater() {
    const next = !laterOpen;
    setLaterOpen(next);
    writeFlag(LATER_KEY, next);
  }

  return (
    <aside className="sidebar">
      <nav className="sidebar-nav" aria-label="Main navigation">
        {groups.map((group) => {
          if (group.collapsible) {
            const contentId = `sidebar-group-${group.id}`;
            return (
              <section
                key={group.id}
                className="sidebar-section"
                data-collapsible
                aria-labelledby={`sidebar-label-${group.id}`}
              >
                <button
                  type="button"
                  className="sidebar-section-toggle"
                  aria-expanded={laterOpen}
                  aria-controls={contentId}
                  onClick={toggleLater}
                >
                  <ChevronRight
                    className="sidebar-chevron"
                    size={12}
                    aria-hidden="true"
                    data-open={laterOpen || undefined}
                  />
                  <span id={`sidebar-label-${group.id}`} className="sidebar-section-label">
                    {group.label}
                  </span>
                </button>

                <div id={contentId} className="sidebar-section-items" hidden={!laterOpen}>
                  {(group.subgroups ?? []).map((subgroup) => (
                    <div key={subgroup.id} className="sidebar-subsection">
                      <div className="sidebar-subsection-label">{subgroup.label}</div>
                      <GroupItems group={subgroup} />
                    </div>
                  ))}
                </div>
              </section>
            );
          }

          return (
            <section
              key={group.id}
              className="sidebar-section"
              aria-labelledby={`sidebar-label-${group.id}`}
            >
              <div id={`sidebar-label-${group.id}`} className="sidebar-section-label">
                {group.label}
              </div>
              <GroupItems group={group} />
            </section>
          );
        })}
      </nav>

      <div className="sidebar-footer">
        <button
          type="button"
          className="sidebar-collapse"
          onClick={toggleCollapsed}
          title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-expanded={!collapsed}
        >
          {collapsed ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}
          <span className="sidebar-item-label">{collapsed ? "Expand" : "Collapse"}</span>
        </button>
      </div>
    </aside>
  );
}
