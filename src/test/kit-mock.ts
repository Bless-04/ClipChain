// Vitest runs without SvelteKit's virtual modules. These stand-ins let route
// tests exercise ClipChain's UI; SvelteKit navigation itself is not simulated.
export const resolve = (path: string) => path;
export const page = { url: new URL('http://localhost/') };
