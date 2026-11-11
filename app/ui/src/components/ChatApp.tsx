import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';

import {
  chatDelete,
  chatHistory,
  chatNew,
  chatOpenSaved,
  chatResume,
  chatSend,
  chatStop,
  closeChat,
  getChat,
  openSettings,
} from '../ipc/commands';
import { isTauri } from '../ipc/env';
import { onChatEvent, onChatOpened, onChatOver, onSettingsChanged } from '../ipc/events';
import { chatHistoryMock, chatPageMock, chatSeedMock, openedChatMock } from '../ipc/mock';
import type { ChatSummary, ChatView, OpenedChat } from '../ipc/types';
import { Button, ConfirmButton, ListButton, TextArea } from '../kit';
import {
  applyEvent,
  ask,
  coldText,
  costLabel,
  finish,
  fromSaved,
  isWorking,
  overText,
  splitAnswer,
  toolLabel,
  whenLabel,
  type Block,
  type Exchange,
} from './chatLog';
import { NextCard } from './NextCard';
import { Brand } from './Tooltip';

// "What next" is the card above them, so these go past it.
const SUGGESTIONS = [
  'Explain these changes',
  'Is my damage fine for this act?',
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

function Empty({
  view,
  reload,
  onAsk,
}: {
  view: ChatView | null;
  reload: number;
  onAsk: (q: string) => void;
}): React.JSX.Element {
  if (!view) return <div />;
  if (!view.available) {
    return (
      <p className="font-body text-[13px] text-label italic">
        This build of Exile Companion doesn't include the assistant.
      </p>
    );
  }
  return (
    <div className="flex flex-col">
      <NextCard key={reload} />
      <div className="tooltip-sep my-4" style={{ '--tint': 'var(--color-currency)' } as React.CSSProperties} />
      <AskPrompt view={view} onAsk={onAsk} />
    </div>
  );
}

function AskPrompt({ view, onAsk }: { view: ChatView; onAsk: (q: string) => void }): React.JSX.Element {
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

type Page =
  | { kind: 'chat' }
  | { kind: 'history'; chats: ChatSummary[] | null }
  | { kind: 'saved'; chat: OpenedChat; error: string | null };

const SEP = { '--tint': 'var(--color-currency)' } as React.CSSProperties;

function History({
  chats,
  onOpen,
}: {
  chats: ChatSummary[] | null;
  onOpen: (id: string) => void;
}): React.JSX.Element {
  if (!chats) return <div />;
  return (
    <div className="flex flex-col gap-2">
      <h2 className="font-caps text-[13px] text-label">Past chats</h2>
      {chats.length === 0 ? (
        <p className="font-body text-[13px] text-label italic">
          No saved chats yet. A chat is kept here after its first answer.
        </p>
      ) : (
        <ul className="flex flex-col">
          {chats.map((c) => (
            <li key={c.id}>
              <ListButton onClick={() => onOpen(c.id)}>
                <span className="w-full truncate font-caps text-[14px] text-value">{c.title || 'Untitled chat'}</span>
                <span className="w-full truncate font-body text-[11px] text-label">
                  {[
                    whenLabel(c.updated),
                    c.model,
                    `${c.questions} ${c.questions === 1 ? 'question' : 'questions'}`,
                    costLabel(c.dollars),
                  ]
                    .filter(Boolean)
                    .join(' · ')}
                </span>
              </ListButton>
            </li>
          ))}
        </ul>
      )}
      <p className="font-body text-[12px] leading-snug text-label italic">
        Chats are kept on this PC only.
      </p>
    </div>
  );
}

function Saved({
  chat,
  error,
  onBack,
  onResume,
  onDelete,
}: {
  chat: OpenedChat;
  error: string | null;
  onBack: () => void;
  onResume: () => void;
  onDelete: () => void;
}): React.JSX.Element {
  const cold = coldText(chat.warmth);
  return (
    <div className="flex flex-col">
      <div className="flex flex-col gap-2">
        <div className="flex items-center gap-2">
          <Button onClick={onBack}>All chats</Button>
          <h2 className="min-w-0 truncate font-caps text-[13px] text-label" title={chat.title}>
            {chat.title}
          </h2>
        </div>
        {chat.blocked ? (
          <p className="font-body text-[12px] leading-snug text-label">{chat.blocked}</p>
        ) : (
          cold && <p className="font-body text-[12px] leading-snug text-plain">{cold}</p>
        )}
        <div className="flex items-center gap-2">
          {!chat.blocked && (
            <Button onClick={onResume}>{cold ? 'Continue anyway' : 'Continue'}</Button>
          )}
          <ConfirmButton label="Delete" confirm="Click again to delete" onConfirm={onDelete} />
        </div>
        {error && (
          <p role="alert" className="font-body text-[12px] text-corrupted">
            {error}
          </p>
        )}
      </div>
      {fromSaved(chat.exchanges).map((e, i) => (
        <div key={i}>
          <div className="tooltip-sep my-4" style={SEP} />
          <ExchangeView exchange={e} />
        </div>
      ))}
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

  const [reload, setReload] = useState(0);
  const [page, setPage] = useState<Page>(() => {
    const start = isTauri() ? null : chatPageMock();
    if (start === 'history') return { kind: 'history', chats: chatHistoryMock() };
    if (start === 'saved') return { kind: 'saved', chat: openedChatMock('chat-2'), error: null };
    return { kind: 'chat' };
  });
  const inChat = page.kind === 'chat';

  const showHistory = useCallback(() => {
    setPage({ kind: 'history', chats: null });
    void chatHistory().then((chats) => setPage({ kind: 'history', chats }));
  }, []);

  const openSaved = useCallback((id: string) => {
    void chatOpenSaved(id).then((chat) => setPage({ kind: 'saved', chat, error: null }));
  }, []);

  const refresh = useCallback(() => {
    void getChat().then(setView);
    input.current?.focus();
  }, []);

  useEffect(() => {
    refresh();
    const opened = (): void => {
      refresh();
      // The build or the character may have moved on since the window was last open.
      setReload((n) => n + 1);
    };
    const offs = [
      onChatEvent((e) => setLog((l) => applyEvent(l, e))),
      onChatOver((o) => setLog((l) => finish(l, o))),
      ...(isTauri() ? [onChatOpened(opened), onSettingsChanged(refresh)] : []),
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

  const resume = (id: string): void => {
    chatResume(id)
      .then((saved) => {
        setLog(fromSaved(saved));
        setPage({ kind: 'chat' });
        stuck.current = true;
        input.current?.focus();
      })
      .catch((e: unknown) =>
        setPage((p) => (p.kind === 'saved' ? { ...p, error: String(e) } : p)),
      );
  };

  const remove = (id: string): void => {
    chatDelete(id)
      .then(() => {
        if (log.length && page.kind === 'saved' && page.chat.id === id) setLog([]);
        showHistory();
      })
      .catch((e: unknown) =>
        setPage((p) => (p.kind === 'saved' ? { ...p, error: String(e) } : p)),
      );
  };

  const send = (question: string): void => {
    const q = question.trim();
    if (!q || working || !ready || !inChat) return;
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
          aria-pressed={!inChat}
          onClick={() => (inChat ? showHistory() : setPage({ kind: 'chat' }))}
        >
          History
        </Button>
        <Button
          className="shrink-0 cursor-default"
          disabled={inChat && log.length === 0}
          onClick={() => {
            void chatNew();
            setLog([]);
            setPage({ kind: 'chat' });
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
          {page.kind === 'history' ? (
            <History chats={page.chats} onOpen={openSaved} />
          ) : page.kind === 'saved' ? (
            <Saved
              chat={page.chat}
              error={page.error}
              onBack={showHistory}
              onResume={() => resume(page.chat.id)}
              onDelete={() => remove(page.chat.id)}
            />
          ) : log.length === 0 ? (
            <Empty view={view} reload={reload} onAsk={send} />
          ) : (
            log.map((e, i) => (
              <div key={i}>
                {i > 0 && <div className="tooltip-sep my-4" style={SEP} />}
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
            disabled={!ready || !inChat}
            placeholder={ready && inChat ? 'Ask the assistant' : ''}
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
            <Button className="h-[31px]" disabled={!ready || !inChat || !draft.trim()} onClick={() => send(draft)}>
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
