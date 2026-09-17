import { errorMessage } from '$lib/services/result';
/** Confirmed data with versioned reads and serialized mutations. Events during a
 * request are coalesced into a fresh read after it settles. Failed reads retain data. */
export class Resource<T> {
  value = $state<T>();
  loading = $state(false);
  saving = $state(false);
  readError = $state<string | null>(null);
  mutationError = $state<string | null>(null);
  error = $derived(this.mutationError ?? this.readError);
  private version = 0;
  private pending = false;
  constructor(private readonly read: () => Promise<T>, initial?: T) { this.value = initial; }
  refresh(): void {
    if (this.loading || this.saving) this.pending = true; else void this.load(true);
  }
  async load(preserveMutationError = false): Promise<void> {
    if (this.saving) { this.pending = true; return; }
    if (!preserveMutationError) this.mutationError = null;
    const version = ++this.version;
    this.loading = true; this.readError = null;
    try {
      const result = await this.read();
      if (version === this.version) this.value = result;
    } catch (error: unknown) {
      if (version === this.version) this.readError = errorMessage(error);
    } finally {
      if (version === this.version) { this.loading = false; this.flush(); }
    }
  }
  confirm(value: T): void {
    ++this.version; this.value = value; this.loading = false; this.readError = null; this.flush();
  }
  async mutate<R>(run: () => Promise<R>, apply: (result: R) => T): Promise<R | undefined> {
    if (this.saving) return undefined;
    ++this.version; this.loading = false; this.saving = true; this.readError = null; this.mutationError = null;
    try {
      const result = await run();
      this.value = apply(result);
      return result;
    } catch (error: unknown) { this.mutationError = errorMessage(error); return undefined; }
    finally { this.saving = false; this.flush(); }
  }
  private flush(): void {
    if (this.pending && !this.saving) { this.pending = false; void this.load(true); }
  }
  cancelReads(): void { ++this.version; this.loading = false; this.pending = false; }
}
