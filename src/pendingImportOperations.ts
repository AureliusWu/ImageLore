export function trackImportOperation(
  pending: Set<Promise<void>>,
  operation: () => Promise<void>,
): Promise<void> {
  // Publish ownership before the operation's first flush, dialog or native call.
  const raw = Promise.resolve().then(operation);
  pending.add(raw);
  // The caller handles this rejection; close retains the uncaught raw result.
  return raw.finally(() => pending.delete(raw));
}

export async function waitForImportCloseOperations(
  operations: Array<Promise<unknown> | null | undefined>,
): Promise<void> {
  const results = await Promise.allSettled(operations);
  // Keep ingress closed until every owned write finishes, including after failure.
  for (const result of results) {
    if (result.status === "rejected") throw result.reason;
  }
}
