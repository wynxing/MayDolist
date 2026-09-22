const NUMBER_TOKEN = /^#(\d+)$/;

export function githubQueryTokens(query: string): string[] {
  return query.trim().toLowerCase().split(/\s+/).filter(Boolean);
}

function segments(fullName: string): string[] {
  return fullName.toLowerCase().split("/").filter(Boolean);
}

/** Repo selector: `owner/repo`, a full owner or repo segment, or its first 4+ letters. */
function selectsRepo(token: string, repoNames: readonly string[]): boolean {
  if (token.includes("/")) {
    return repoNames.some((name) => name.toLowerCase().includes(token));
  }
  return repoNames.some((name) => segmentHit(name, token));
}

function segmentHit(fullName: string, token: string): boolean {
  return segments(fullName).some(
    (segment) => segment === token || (token.length >= 4 && segment.startsWith(token))
  );
}

function repoSelected(token: string, repoFullName: string): boolean {
  if (token.includes("/")) return repoFullName.toLowerCase().includes(token);
  return segmentHit(repoFullName, token);
}

export function matchesGithubItem(
  query: string,
  repoNames: readonly string[],
  repoFullName: string,
  title: string,
  number: number
): boolean {
  const tokens = githubQueryTokens(query);
  if (!tokens.length) return true;
  const titleHaystack = `${title} #${number}`.toLowerCase();
  return tokens.every((token) => {
    const exactNumber = NUMBER_TOKEN.exec(token);
    if (exactNumber) return number === Number(exactNumber[1]);
    const titleHit = titleHaystack.includes(token);
    if (!selectsRepo(token, repoNames)) return titleHit;
    return repoSelected(token, repoFullName) || titleHit;
  });
}

/** Repo card stays when the query only names that repo, even if it has no loaded items. */
export function repoMatchesQuery(
  query: string,
  repoNames: readonly string[],
  repoFullName: string
): boolean {
  const tokens = githubQueryTokens(query);
  if (!tokens.length) return true;
  if (tokens.some((token) => NUMBER_TOKEN.test(token) || !selectsRepo(token, repoNames))) {
    return false;
  }
  return tokens.every((token) => repoSelected(token, repoFullName));
}
