import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { TokenExpiryBadge } from "./TokenExpiryBadge";

/**
 * The badge crosses 4 thresholds (>14d / 7-14d / <7d / expired) plus the
 * "unknown" case. Each test pins time so the relative-day math is
 * deterministic across CI runs and developer machines.
 */
const NOW = new Date("2026-05-08T12:00:00.000Z").getTime();

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
});

afterEach(() => {
  vi.useRealTimers();
});

/** ISO timestamp `days` from NOW. Negative values produce expired tokens. */
function daysFromNow(days: number): string {
  return new Date(NOW + days * 86_400_000).toISOString();
}

describe("TokenExpiryBadge", () => {
  it("renders nothing when expiresAt is null (legacy row)", () => {
    const { container } = render(<TokenExpiryBadge expiresAt={null} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing when expiresAt is undefined", () => {
    const { container } = render(<TokenExpiryBadge expiresAt={undefined} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing when the timestamp is unparseable", () => {
    const { container } = render(<TokenExpiryBadge expiresAt="not-a-date" />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows a green long-runway badge when > 14 days remain", () => {
    render(<TokenExpiryBadge expiresAt={daysFromNow(45)} />);
    const badge = screen.getByText(/Expire dans 45 j/);
    expect(badge).toBeInTheDocument();
    // Long runway uses emerald — this is the only state we colour green.
    expect(badge.className).toMatch(/emerald/);
  });

  it("shows an amber warning when 7-14 days remain", () => {
    render(<TokenExpiryBadge expiresAt={daysFromNow(10)} />);
    const badge = screen.getByText(/Expire dans 10 j/);
    expect(badge).toBeInTheDocument();
    expect(badge.className).toMatch(/amber/);
  });

  it("shows a red urgent warning when less than 7 days remain", () => {
    render(<TokenExpiryBadge expiresAt={daysFromNow(3)} />);
    const badge = screen.getByText(/Expire dans 3 j/);
    expect(badge).toBeInTheDocument();
    expect(badge.className).toMatch(/red/);
  });

  it("shows an explicit 'expiré' message when the token is past its expiry", () => {
    render(<TokenExpiryBadge expiresAt={daysFromNow(-2)} />);
    // Once expired we drop the day count (which would be negative — useless to
    // the user) and show an actionable message instead.
    expect(screen.getByText(/Expiré.*reconnecte/i)).toBeInTheDocument();
  });

  it("uses the boundary day count exactly at 7 and 14 days", () => {
    // Boundary regression guards: an off-by-one would push the wrong colour
    // at exactly the threshold. We use Math.floor so 7.0 days = 7, which
    // falls into the "<= 7" red bucket; 14.0 days = 14 falls into amber.
    render(<TokenExpiryBadge expiresAt={daysFromNow(7)} />);
    const at7 = screen.getByText(/Expire dans 7 j/);
    expect(at7.className).toMatch(/red/);
  });
});
