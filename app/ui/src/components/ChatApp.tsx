import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';

import { chatNew, chatSend, chatStop, closeChat, getChat, openSettings } from '../ipc/commands';
import { isTauri } from '../ipc/env';
import { onChatEvent, onChatOpened, onChatOver, onSettingsChanged } from '../ipc/events';
import { chatSeedMock } from '../ipc/mock';
import type { ChatView } from '../ipc/types';
import { Button, TextArea } from '../kit';
import {
  applyEvent,
  ask,
  costLabel,
  finish,
  isWorking,
  overText,
  splitAnswer,
  toolLabel,
  type Block,
  type Exchange,
} from './chatLog';
import { Brand } from './Tooltip';

const SUGGESTIONS = [
  'What should I do next?',
  'Which passive should I take next?',
  'Are my defences fine for this act?',
];

function Blocks({ blocks, className }: { blocks: Block[]; className: string }): React.JSX.Element {
  return (
    <>
      {blocks.map((b, i) =>
        b.kind === 'text' ? (
          <p key={i} className={`whitespace-pre-line ${className}`}>
            {b.text}
          </p>
        ) : (
          <ul key={i} className={`flex flex-col gap-0.5 ${className}`}>
            {b.items.map((item, j) => (
              <li key={j} className="chat-item">
                {item}
              </li>
            ))}
          </ul>
        ),
      )}
    </>
  );
}

function Working({ exchange }: { exchange: Exchange }): React.JSX.Element {
  return (
    <div role="status" className="flex flex-col gap-0.5">
      <p className="flex items-center gap-2 font-caps text-[13px] text-plain">
        <span className="chat-pulse" aria-hidden="true" />
        {toolLabel(exchange.tool)}…
      </p>
      {exchange.note && <p className="pl-4 font-body text-[13px] text-label italic">{exchange.note}</p>}
    </div>
  );
}

function ExchangeView({ exchange }: { exchange: Exchange }): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const { lead, details } = splitAnswer(exchange.answer);
  const over = exchange.over;
  const problem = over && overText(over);
  const meta = over && [over.model, costLabel(over.dollars)].filter(Boolean).join(' · ');
  return (
    <article className="flex flex-col gap-2">
      <p className="line-clamp-3 font-body text-[13px] whitespace-pre-line text-label" title={exchange.question}>
        {exchange.question}
      </p>
      <div className="flex flex-col gap-1.5 select-text">
        <Blocks blocks={lead} className="font-caps text-[15px] leading-snug text-value" />
        {details.length > 0 && open && (
          <Blocks blocks={details} className="font-body text-[14px] leading-snug text-plain" />
        )}
      </div>
      {!over && (!exchange.answer || exchange.tool || exchange.note) && <Working exchange={exchange} />}
      {(details.length > 0 || problem || meta) && (
        <div className="flex min-h-6 items-center gap-3">
          {details.length > 0 && (
            <Button aria-expanded={open} onClick={() => setOpen(!open)}>
              {open ? 'Less' : 'Details'}
            </Button>
          )}
          {problem && (
            <span role="alert" className={`font-body text-[12px] ${over?.stop === 'stopped' ? 'text-label' : 'text-corrupted'}`}>
              {problem}
            </span>
          )}
          {meta && !problem && (
            <span className="ml-auto truncate font-body text-[11px] text-label">{meta}</span>
          )}
        </div>
      )}
    </article>
  );
}

function Empty({ view, onAsk }: { view: ChatView | null; onAsk: (q: string) => void }): React.JSX.Element {
  if (!view) return <div />;
  if (!view.available) {
    return (
      <p className="font-body text-[13px] text-label italic">
        This build of Exile Companion doesn't include the assistant.
      </p>
    );
  }
  if (view.missing) {
    return (
      <div className="flex flex-col items-start gap-3">
        <p className="font-caps text-[15px] leading-snug text-value">{view.missing}</p>
        <Button onClick={() => void openSettings()}>Open settings</Button>
        <p className="font-body text-[12px] leading-snug text-label italic">
          The assistant runs on your own key from Anthropic or OpenRouter, or on a model on your own
          server. The provider bills you for each answer, usually a few cents.
        </p>
      </div>
    );
  }
  return (
    <div className="flex flex-col items-start gap-3">
      <p className="font-caps text-[15px] leading-snug text-value">Ask about your build.</p>
      <p className="font-body text-[13px] leading-snug text-label italic">
        It measures with your Path of Building and knows where you are in the campaign. Paste an item
        with Ctrl+V to see if it's an upgrade.
      </p>
      <div className="flex flex-col items-start gap-1.5">
        {SUGGESTIONS.map((s) => (
          <Button key={s} onClick={() => onAsk(s)}>
            {s}
          </Button>
        ))}
      </div>
    </div>
  );
}

export function ChatApp(): React.JSX.Element {
  const [view, setView] = useState<ChatView | null>(null);
  const [log, setLog] = useState<Exchange[]>(() => (isTauri() ? [] : chatSeedMock()));
  const [draft, setDraft] = useState('');
  const input = useRef<HTMLTextAreaElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const stuck = useRef(true);
  const working = isWorking(log);
  const ready = !!view?.available && !view.missing;

  const refresh = useCallback(() => {
    void getChat().then(setView);
    input.current?.focus();
  }, []);

  useEffect(() => {
    refresh();
    const offs = [
      onChatEvent((e) => setLog((l) => applyEvent(l, e))),
      onChatOver((o) => setLog((l) => finish(l, o))),
      ...(isTauri() ? [onChatOpened(refresh), onSettingsChanged(refresh)] : []),
    ];
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') void closeChat();
    };
    window.addEventListener('keydown', onKey);
    return () => {
      offs.forEach((off) => void off.then((f) => f()));
      window.removeEventListener('keydown', onKey);
    };
  }, [refresh]);

  // Follows the answer down, unless the player scrolled up to read.
  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && stuck.current) el.scrollTop = el.scrollHeight;
  });

  const send = (question: string): void => {
    const q = question.trim();
    if (!q || working || !ready) return;
    stuck.current = true;
    setLog((l) => ask(l, q));
    setDraft('');
    chatSend(q).catch((e: unknown) =>
      setLog((l) =>
        finish(l, { stop: 'error', message: String(e), usage: null, dollars: null, model: null }),
      ),
    );
  };

  return (
    <div className="flex h-full flex-col">
      <header
        data-tauri-drag-region
        className="flex h-8 shrink-0 cursor-move items-center gap-2 border-b border-edge bg-black pr-1 pl-3"
      >
        <span data-tauri-drag-region className="font-caps text-[13px] text-normal">
          <Brand />
        </span>
        <span data-tauri-drag-region className="font-caps text-[13px] text-label">
          Assistant
        </span>
        {ready && (
          <span data-tauri-drag-region className="min-w-0 truncate font-body text-[11px] text-label">
            {view?.model}
          </span>
        )}
        <Button
          className="ml-auto shrink-0 cursor-default"
          disabled={log.length === 0}
          onClick={() => {
            void chatNew();
            setLog([]);
            input.current?.focus();
          }}
        >
          New chat
        </Button>
        <Button
          className="grid size-6 cursor-default place-items-center p-0"
          aria-label="Close the assistant"
          onClick={() => void closeChat()}
        >
          <svg viewBox="0 0 12 12" className="size-3" aria-hidden="true">
            <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" fill="none" stroke="currentColor" strokeWidth="1.4" />
          </svg>
        </Button>
      </header>
      <div
        ref={scroller}
        className="chat-scroll min-h-0 flex-1 overflow-y-auto"
        onScroll={(e) => {
          const el = e.currentTarget;
          stuck.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
        }}
      >
        <main className="flex flex-col px-4 py-4" aria-live="polite">
          {log.length === 0 ? (
            <Empty view={view} onAsk={send} />
          ) : (
            log.map((e, i) => (
              <div key={i}>
                {i > 0 && <div className="tooltip-sep my-4" style={{ '--tint': 'var(--color-currency)' } as React.CSSProperties} />}
                <ExchangeView exchange={e} />
              </div>
            ))
          )}
        </main>
      </div>
      <footer className="flex shrink-0 flex-col gap-1.5 border-t border-edge bg-black/60 px-3 pt-2.5 pb-2">
        <div className="flex items-end gap-2">
          <TextArea
            ref={input}
            value={draft}
            maxRows={6}
            disabled={!ready}
            placeholder={ready ? 'Ask the assistant' : ''}
            aria-label="Question for the assistant"
            className="flex-1"
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                send(draft);
              }
            }}
          />
          {working ? (
            <Button className="h-[31px]" onClick={() => void chatStop()}>
              Stop
            </Button>
          ) : (
            <Button className="h-[31px]" disabled={!ready || !draft.trim()} onClick={() => send(draft)}>
              Ask
            </Button>
          )}
        </div>
        <p className="font-body text-[11px] text-label">
          Enter asks. Shift+Enter for a new line. Esc closes and the answer goes on.
        </p>
      </footer>
    </div>
  );
}
