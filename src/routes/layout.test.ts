import { mockAvailable, mockCommands, mockEvents } from '../test/desktop-mock';
import { beforeEach, it, expect, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import { page } from '../test/kit-mock';
import { settings, trimRule } from '../test/fixtures';
import { AppState } from '$lib/state/app.svelte';
import { ROUTES } from '$lib/constants/routes';
import Layout from './+layout.svelte';

let app: AppState;
vi.mock('$lib/state/app.svelte', async (original) => ({
  ...await original<typeof import('$lib/state/app.svelte')>(), provideApp: () => app,
}));
const children = createRawSnippet(() => ({ render: () => '<p>Route content</p>' }));
beforeEach(() => {
  app = new AppState(); page.url = new URL('http://localhost/');
  mockCommands.getRules.mockResolvedValue({ status: 'ok', data: [] });
  mockCommands.getHistory.mockResolvedValue({ status: 'ok', data: [] });
  mockCommands.getSettings.mockResolvedValue({ status: 'ok', data: settings });
  mockCommands.getMonitorStatus.mockResolvedValue({ status: 'ok', data: { isRunning: true, lastError: null } });
});

it('renders all navigation destinations, the current route, and an accessible main target', () => {
  mockAvailable.mockReturnValue(false);
  page.url = new URL('http://localhost' + ROUTES.SETTINGS);
  app.rules.confirm([trimRule(1), trimRule(2)]);
  render(Layout, { children });
  const navigation = within(screen.getByRole('navigation', { name: 'Main navigation' }));
  for (const [name, href] of [['Overview', ROUTES.DASHBOARD], ['Rules', ROUTES.RULES], ['History', ROUTES.HISTORY], ['Settings', ROUTES.SETTINGS]]) {
    expect(navigation.getByRole('link', { name: new RegExp(name!) }), "Each navigation link must point to its documented route.").toHaveAttribute('href', href);
  }
  expect(navigation.getByRole('link', { name: 'Settings' }), "The current Settings page must be identified to assistive technology.").toHaveAttribute('aria-current', 'page');
  expect(navigation.getByRole('link', { name: /Rules/ }), "The Rules navigation badge must reflect the saved rule count.").toHaveTextContent('2');
  expect(screen.getByRole('link', { name: 'Skip to content' }), "The skip link must target the main content region for keyboard navigation.").toHaveAttribute('href', '#main-content');
  expect(screen.getByRole('main'), "The main region must expose the ID used by the skip link.").toHaveAttribute('id', 'main-content');
  expect(screen.getByText('Route content'), "The layout must render the active page's content.").toBeInTheDocument();
});

it('shows browser limitations and avoids initializing native data', () => {
  mockAvailable.mockReturnValue(false);
  render(Layout, { children });
  expect(screen.getByText('Browser preview'), "A browser session must be visibly labeled as a preview.").toBeInTheDocument();
  expect(screen.getByText(/Clipboard monitoring and saved data are available/), "The browser notice must explain that clipboard monitoring and persistence require the desktop app.").toBeInTheDocument();
  Object.values(mockCommands).forEach((command) => expect(command, "Browser preview must not call native data commands.").not.toHaveBeenCalled());
  Object.values(mockEvents).forEach((event) => expect(event.listen, "Browser preview must not subscribe to native events.").not.toHaveBeenCalled());
});

it('reconnects live updates from the visible Retry action', async () => {
  mockEvents.rulesUpdated.listen.mockRejectedValueOnce(new Error('Bridge unavailable'));
  render(Layout, { children });
  expect(await screen.findByRole('alert'), "A failed event subscription must show a live-update warning.").toHaveTextContent('Live updates unavailable');
  await userEvent.setup().click(screen.getByRole('button', { name: 'Retry' }));
  await waitFor(() => expect(mockEvents.monitorChanged.listen, "Retry must finish installing the missing native listeners.").toHaveBeenCalledOnce());
  expect(screen.queryByRole('alert'), "A successful reconnection must remove the live-update warning.").not.toBeInTheDocument();
  expect(screen.queryByText('Browser preview'), "A temporary event failure must not mislabel a desktop session as browser preview.").not.toBeInTheDocument();
});

it('releases every native listener when the layout is unmounted', async () => {
  const releases = Object.values(mockEvents).map((event) => {
    const release = vi.fn(); event.listen.mockResolvedValue(release); return release;
  });
  const { unmount } = render(Layout, { children });
  await waitFor(() => expect(app.status, "The layout fixture must finish startup before testing teardown.").toBe('Monitoring clipboard'));
  unmount();
  releases.forEach((release) => expect(release, "Unmounting the layout must release every native listener exactly once.").toHaveBeenCalledOnce());
});
