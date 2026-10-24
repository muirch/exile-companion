import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import * as commands from '../ipc/commands';
import { useAppStore } from '../store/useAppStore';
import { SettingsApp, presetName } from './SettingsApp';

vi.mock('../ipc/commands', async (importOriginal) => {
  const real = await importOriginal<typeof commands>();
  const stubs = Object.fromEntries(
    Object.keys(real).map((name) => [name, vi.fn(() => Promise.resolve(null))]),
  );
  return {
    ...stubs,
    getHotkeys: vi.fn(() => new Promise(() => {})),
  };
});

describe('presetName', () => {
  it('strips the dashes PoB separators are drawn with', () => {
    expect(presetName('----- Mapping -----', 1)).toBe('Mapping');
    expect(presetName('=== ===', 2)).toBe('Preset 3');
    expect(presetName('Act 3 {2}', 0)).toBe('Act 3 {2}');
  });
});

describe('SettingsApp', () => {
  it('lets the player set the act of each preset of the build', async () => {
    act(() =>
      useAppStore.setState({
        build: {
          class_name: 'Ranger',
          ascend_class_name: null,
          level: 90,
          active_spec_title: null,
          main_skill: null,
          spec_count: 1,
          gem_count: 3,
          not_in_campaign: ['Vaal Haste', 'Empower Support'],
          mules: [
            {
              class: 'Shadow',
              take: { name: 'Cobra Lash', npc: 'Tarkleigh', own_act: 3 },
              buy: [{ name: 'Momentum', npc: 'Nessa', own_act: 3 }],
            },
          ],
          presets: [
            { title: 'Leveling', gem_count: 2, auto_level: null },
            { title: '-- separator --', gem_count: 0, auto_level: null },
            { title: 'Switch', gem_count: 1, auto_level: null },
          ],
        },
      }),
    );
    render(<SettingsApp />);
    expect(screen.getByText('No gems')).toBeInTheDocument();
    expect(screen.getByText('Vaal Haste, Empower Support')).toBeInTheDocument();
    expect(screen.getByText('Shadow: take Cobra Lash, buy Momentum from Nessa')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('combobox', { name: 'Stage of Switch' }));
    await userEvent.click(screen.getByRole('option', { name: 'From act 4' }));
    expect(commands.updateSettings).toHaveBeenCalledWith({
      presetStages: ['auto', 'auto', { act: 4 }],
    });
    act(() => useAppStore.setState({ build: null }));
  });

  it('shows the Path of Building folder it found and its version', async () => {
    vi.mocked(commands.getPob).mockResolvedValueOnce({
      path: 'C:\\PoB',
      version: '2.67.2',
    });
    render(<SettingsApp />);
    expect(await screen.findByText('C:\\PoB')).toBeInTheDocument();
    expect(screen.getByText('2.67.2')).toBeInTheDocument();
  });

  it('says when there is no Path of Building and why a picked folder was refused', async () => {
    vi.mocked(commands.pickPobFolder).mockRejectedValueOnce(
      "That's Path of Building for Path of Exile 2. Pick the one for Path of Exile 1.",
    );
    render(<SettingsApp />);
    expect(await screen.findByText('Not found')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Pick PoB folder' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Path of Exile 2');
  });

  describe('Sync with PoB', () => {
    const pob = { path: 'C:\\PoB', version: '2.67.2' };
    const builds = 'C:\\Users\\me\\Documents\\Path of Building\\Builds\\';
    const off = { enabled: false, file: `${builds}Exile Companion.xml`, picked: false };

    it('names the PoB build it keeps in step once turned on', async () => {
      vi.mocked(commands.getPob).mockResolvedValueOnce(pob);
      vi.mocked(commands.getPobSync).mockResolvedValueOnce(off);
      vi.mocked(commands.setPobSync).mockResolvedValueOnce({ ...off, enabled: true });
      render(<SettingsApp />);
      const toggle = await screen.findByRole('switch', { name: 'Sync with PoB' });
      expect(toggle).toHaveTextContent('Off');
      expect(screen.queryByTitle(`${builds}Exile Companion.xml`)).not.toBeInTheDocument();
      await userEvent.click(toggle);
      expect(commands.setPobSync).toHaveBeenCalledWith(true);
      expect(await screen.findByTitle(`${builds}Exile Companion.xml`)).toHaveTextContent(/^Exile Companion$/);
      expect(screen.queryByRole('button', { name: 'Use Exile Companion' })).not.toBeInTheDocument();
    });

    it('follows a picked PoB build and can go back to its own', async () => {
      vi.mocked(commands.getPob).mockResolvedValueOnce(pob);
      vi.mocked(commands.getPobSync).mockResolvedValueOnce(off);
      vi.mocked(commands.pickPobBuild).mockResolvedValueOnce({
        enabled: true,
        file: `${builds}PConc.xml`,
        picked: true,
      });
      vi.mocked(commands.unpickPobBuild).mockResolvedValueOnce({ ...off, enabled: true });
      render(<SettingsApp />);
      await userEvent.click(await screen.findByRole('button', { name: 'Pick PoB build' }));
      expect(await screen.findByTitle(`${builds}PConc.xml`)).toHaveTextContent('PConc');
      await userEvent.click(screen.getByRole('button', { name: 'Use Exile Companion' }));
      expect(await screen.findByTitle(`${builds}Exile Companion.xml`)).toBeInTheDocument();
    });

    it('says why PoB could not be opened', async () => {
      vi.mocked(commands.getPob).mockResolvedValueOnce(pob);
      vi.mocked(commands.getPobSync).mockResolvedValueOnce(off);
      vi.mocked(commands.openPob).mockRejectedValueOnce('There\'s no Path of Building.exe in C:\\PoB.');
      render(<SettingsApp />);
      await userEvent.click(await screen.findByRole('button', { name: 'Open PoB' }));
      expect(await screen.findByRole('alert')).toHaveTextContent('Path of Building.exe');
    });
  });

  describe('Assistant', () => {
    const view = {
      server: 'C:\\EC\\exile-companion-mcp.exe',
      agents: [
        { id: 'claudeDesktop' as const, name: 'Claude Desktop', status: { state: 'connected' as const }, config: '' },
        { id: 'cursor' as const, name: 'Cursor', status: { state: 'notConnected' as const }, config: '' },
        { id: 'codex' as const, name: 'Codex', status: { state: 'notFound' as const }, config: '' },
      ],
    };

    it('connects an agent the player has', async () => {
      vi.mocked(commands.getAssistant).mockResolvedValueOnce(view);
      vi.mocked(commands.connectAgent).mockResolvedValueOnce({
        ...view,
        agents: view.agents.map((a) => (a.id === 'cursor' ? { ...a, status: { state: 'connected' as const } } : a)),
      });
      render(<SettingsApp />);
      expect(await screen.findByText('Not installed')).toBeInTheDocument();
      expect(screen.getAllByText('Connected')).toHaveLength(1);
      expect(screen.queryByRole('button', { name: 'Connect Claude Desktop' })).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Connect Codex' })).not.toBeInTheDocument();

      await userEvent.click(screen.getByRole('button', { name: 'Connect Cursor' }));
      expect(commands.connectAgent).toHaveBeenCalledWith('cursor');
      expect(await screen.findAllByText('Connected')).toHaveLength(2);
    });

    it('says why an agent could not be connected', async () => {
      vi.mocked(commands.getAssistant).mockResolvedValueOnce(view);
      vi.mocked(commands.connectAgent).mockRejectedValueOnce('Left mcp.json as it was, the config file isn\'t valid JSON.');
      render(<SettingsApp />);
      await userEvent.click(await screen.findByRole('button', { name: 'Connect Cursor' }));
      expect(await screen.findByRole('alert')).toHaveTextContent('Left mcp.json as it was');
    });

    it('copies the config for other agents', async () => {
      vi.mocked(commands.getAssistant).mockResolvedValueOnce(view);
      render(<SettingsApp />);
      await userEvent.click(await screen.findByRole('button', { name: 'Copy config' }));
      expect(commands.copyAssistantConfig).toHaveBeenCalled();
      expect(await screen.findByRole('button', { name: 'Copied' })).toBeInTheDocument();
    });

    it('says when the build has no assistant', async () => {
      vi.mocked(commands.getAssistant).mockResolvedValueOnce({ server: null, agents: [] });
      render(<SettingsApp />);
      expect(await screen.findByText("This build of Exile Companion doesn't include the assistant.")).toBeInTheDocument();
    });
  });

  it('shows why a pasted build did not load', async () => {
    vi.mocked(commands.pasteBuild).mockRejectedValueOnce("Couldn't download the build: timeout");
    render(<SettingsApp />);
    await userEvent.click(screen.getByRole('button', { name: 'Paste code or link' }));
    expect(await screen.findByRole('alert')).toHaveTextContent("Couldn't download the build: timeout");
  });

  it('saves which gem steps the route shows', async () => {
    render(<SettingsApp />);
    const rewards = screen.getByRole('button', { name: 'Rewards' });
    expect(screen.getByRole('button', { name: 'Rewards and buys' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await userEvent.click(rewards);
    expect(rewards).toHaveAttribute('aria-pressed', 'true');
    expect(commands.updateSettings).toHaveBeenCalledWith({ gemHints: 'rewards' });
  });

  it('switches the overlay to compact and sets the fade time', async () => {
    render(<SettingsApp />);
    await userEvent.click(screen.getByRole('button', { name: 'Compact' }));
    expect(commands.updateSettings).toHaveBeenCalledWith({ compact: true });
    await userEvent.click(screen.getByRole('button', { name: '10 s' }));
    expect(commands.updateSettings).toHaveBeenCalledWith({ fadeAfter: 10 });
  });

  describe('Start over', () => {
    beforeEach(() => vi.useFakeTimers({ toFake: ['Date'] }));
    afterEach(() => vi.useRealTimers());

    it('asks for a second click before starting the route over', async () => {
      render(<SettingsApp />);
      await userEvent.click(screen.getByRole('button', { name: 'Start over' }));
      expect(commands.routeReset).not.toHaveBeenCalled();
      vi.setSystemTime(Date.now() + 600);
      await userEvent.click(screen.getByRole('button', { name: 'Click again to start over' }));
      expect(commands.routeReset).toHaveBeenCalledTimes(1);
    });

    it('does not take a double click as the second click', async () => {
      render(<SettingsApp />);
      await userEvent.dblClick(screen.getByRole('button', { name: 'Start over' }));
      expect(commands.routeReset).not.toHaveBeenCalled();
      expect(screen.getByRole('button', { name: 'Click again to start over' })).toBeInTheDocument();
    });
  });

  it('keeps log replay behind Testing', async () => {
    render(<SettingsApp />);
    expect(screen.queryByRole('button', { name: 'Replay a log' })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Show' }));
    expect(screen.getByRole('button', { name: 'Replay a log' })).toBeInTheDocument();
  });

  it('closes from its own title bar', async () => {
    render(<SettingsApp />);
    await userEvent.click(screen.getByRole('button', { name: 'Close settings' }));
    expect(commands.closeSettings).toHaveBeenCalled();
  });

  it('moves by the title bar, not by the close button', () => {
    render(<SettingsApp />);
    const bar = screen.getByText('Settings').closest('header');
    expect(bar).toHaveAttribute('data-tauri-drag-region');
    expect(screen.getByRole('button', { name: 'Close settings' })).not.toHaveAttribute(
      'data-tauri-drag-region',
    );
  });
});
