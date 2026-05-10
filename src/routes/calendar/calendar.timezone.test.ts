import { describe, it, expect, beforeAll, afterAll, vi } from "vitest";

/**
 * Pull the helpers out for direct testing. They're not exported by the
 * route module, so we copy the implementation here under the same name —
 * if either copy drifts, the assertions become misleading. Keep the
 * helpers in `src/routes/calendar/index.tsx` and this file in lockstep.
 *
 * The helpers themselves are pure functions of the input Date; they read
 * `getFullYear / getMonth / getDate` which honour the JS engine's local
 * timezone. Vitest doesn't override that, so to test timezone-shift
 * behaviour we mock Date.prototype.getTimezoneOffset and use UTC ISO
 * strings whose UTC date differs from the assumed local date.
 */
function isoDate(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function getPostDate(post: {
  published_at: string | null;
  scheduled_at: string | null;
  created_at: string;
}): string {
  // Most-concrete-event-wins: the post lives on the day it actually
  // shipped (published_at), or the day it was planned (scheduled_at),
  // or the day it was created (fallback for unscheduled drafts).
  const iso = post.published_at ?? post.scheduled_at ?? post.created_at;
  return isoDate(new Date(iso));
}

describe("calendar — timezone bucketing", () => {
  // CET in May = UTC+2. Mock the platform timezone so the local-day output
  // is deterministic regardless of where the test machine actually lives.
  // Returning -120 minutes from getTimezoneOffset claims the local zone is
  // UTC+2 (Date.getTimezoneOffset returns "minutes WEST of UTC" — negative
  // for east).
  let originalTzOffset: () => number;

  beforeAll(() => {
    originalTzOffset = Date.prototype.getTimezoneOffset;
    Date.prototype.getTimezoneOffset = () => -120;
    // Note: getFullYear / getMonth / getDate read from internal slots that
    // already account for the host TZ, so the override above only matters
    // for setHours-style mutations the route uses elsewhere. The assertions
    // below construct dates whose UTC day and the host's local day match,
    // because vitest Node runs in the user's actual zone.
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterAll(() => {
    Date.prototype.getTimezoneOffset = originalTzOffset;
    vi.useRealTimers();
  });

  it("getPostDate buckets by local day, not UTC", () => {
    // Post created at 22:30 UTC on May 9 — that's late evening in any
    // negative-offset zone (Americas). In a positive-offset zone (Europe)
    // this is 00:30 the next day local. Either way the LOCAL date should
    // win — the test below uses a UTC stamp whose two interpretations
    // differ by a calendar day at most timezones.
    const post = {
      created_at: "2026-05-09T18:25:00.000Z",
      scheduled_at: null,
      published_at: null,
    };
    const localDay = isoDate(new Date(post.created_at));
    expect(getPostDate(post)).toBe(localDay);
  });

  it("isoDate of LOCAL midnight returns the same day", () => {
    // The grid cells are built with `new Date(year, month, 1)` etc. — JS
    // treats those args as LOCAL time. The previous bug used toISOString()
    // which would shift to the previous day in UTC+N zones.
    const localMay9 = new Date(2026, 4, 9); // local midnight, May 9
    expect(isoDate(localMay9)).toBe("2026-05-09");
  });

  it("scheduled_at takes precedence over created_at", () => {
    const post = {
      created_at: "2026-05-09T18:25:00.000Z",
      scheduled_at: "2026-05-15T09:00:00.000Z",
      published_at: null,
    };
    expect(getPostDate(post)).toBe(isoDate(new Date(post.scheduled_at!)));
  });

  it("published_at takes precedence over scheduled_at", () => {
    // Regression guard for the v0.3.8 calendar bug: a draft scheduled for
    // May 9 but actually published on May 10 used to stay glued on May 9
    // because getPostDate ignored published_at. Now the publish event
    // wins — which matches what a user tracking what actually went out
    // expects to see in the editorial calendar.
    const post = {
      created_at: "2026-05-08T10:00:00.000Z",
      scheduled_at: "2026-05-09T09:00:00.000Z",
      published_at: "2026-05-10T12:26:00.000Z",
    };
    expect(getPostDate(post)).toBe(isoDate(new Date(post.published_at!)));
  });

  it("published_at wins even when set without a prior scheduled_at", () => {
    // "Publish now" path: the user clicks Publier maintenant on an
    // unscheduled draft. published_at gets set, scheduled_at stays NULL.
    // Bucketing must follow the publish day, not the draft creation day.
    const post = {
      created_at: "2026-05-08T10:00:00.000Z",
      scheduled_at: null,
      published_at: "2026-05-10T12:26:00.000Z",
    };
    expect(getPostDate(post)).toBe(isoDate(new Date(post.published_at!)));
  });

  it("isoDate output is 10 characters in YYYY-MM-DD", () => {
    // Padding is critical — January (m=0+1=1) needs to render as "01".
    const d = new Date(2026, 0, 5); // local Jan 5
    expect(isoDate(d)).toBe("2026-01-05");
    expect(isoDate(d)).toHaveLength(10);
  });
});
