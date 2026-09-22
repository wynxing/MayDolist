import { describe, expect, it } from "vitest";
import { matchesGithubItem, repoMatchesQuery } from "./githubFilter";

const repos = ["wynxing/MayDolist", "vuejs/core"];

function item(
  query: string,
  repo: string,
  title: string,
  number: number,
  names: readonly string[] = repos
) {
  return matchesGithubItem(query, names, repo, title, number);
}

describe("matchesGithubItem", () => {
  it("matches nothing extra when the query is empty", () => {
    expect(item("  ", "wynxing/MayDolist", "Fix login", 12)).toBe(true);
  });

  it("requires every word, in any order, and ignores case", () => {
    expect(item("fix login", "vuejs/core", "Login page fix", 3)).toBe(true);
    expect(item("FIX", "vuejs/core", "Fix login", 3)).toBe(true);
    expect(item("fix logout", "vuejs/core", "Fix login", 3)).toBe(false);
  });

  it("does not treat a short piece of the repo name as a repo filter", () => {
    expect(item("may", "wynxing/MayDolist", "Fix login", 12)).toBe(false);
    expect(item("may", "wynxing/MayDolist", "Maybe later", 13)).toBe(true);
  });

  it("keeps every item when the word is a repo name, and can combine with a title word", () => {
    expect(item("MayDolist", "wynxing/MayDolist", "Fix login", 12)).toBe(true);
    expect(item("MayDolist", "vuejs/core", "Fix login", 3)).toBe(false);
    expect(item("mayd", "wynxing/MayDolist", "Unrelated", 12)).toBe(true);
    expect(item("MayDolist fix", "wynxing/MayDolist", "Fix login", 12)).toBe(true);
    expect(item("MayDolist fix", "wynxing/MayDolist", "Update docs", 13)).toBe(false);
  });

  it("matches owner/repo text against the repo, not the title", () => {
    expect(item("wynxing/may", "wynxing/MayDolist", "Update docs", 12)).toBe(true);
    expect(item("wynxing/may", "vuejs/core", "Update docs", 3)).toBe(false);
  });

  it("matches #number exactly and a bare number as text", () => {
    expect(item("#12", "vuejs/core", "Update docs", 12)).toBe(true);
    expect(item("#12", "vuejs/core", "Update docs", 120)).toBe(false);
    expect(item("#12 fix", "wynxing/MayDolist", "Fix login", 12)).toBe(true);
    expect(item("12", "vuejs/core", "See #120", 9)).toBe(true);
  });
});

describe("repoMatchesQuery", () => {
  it("keeps an empty repo when the query only names it", () => {
    expect(repoMatchesQuery("MayDolist", repos, "wynxing/MayDolist")).toBe(true);
    expect(repoMatchesQuery("wynxing/may", repos, "wynxing/MayDolist")).toBe(true);
    expect(repoMatchesQuery("MayDolist", repos, "vuejs/core")).toBe(false);
  });

  it("does not keep a repo for title words or numbers", () => {
    expect(repoMatchesQuery("fix", repos, "wynxing/MayDolist")).toBe(false);
    expect(repoMatchesQuery("may", repos, "wynxing/MayDolist")).toBe(false);
    expect(repoMatchesQuery("#12", repos, "wynxing/MayDolist")).toBe(false);
    expect(repoMatchesQuery("MayDolist fix", repos, "wynxing/MayDolist")).toBe(false);
  });
});
