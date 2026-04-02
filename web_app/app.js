const ROWS = 6;
const COLS = 7;
const FILES = ["a", "b", "c", "d", "e", "f", "g"];
const RANKS = [6, 5, 4, 3, 2, 1];
const WHITE = "W";
const BLACK = "B";
const BALL = "NB";

const DIRECTIONS = [
  { dr: -1, dc: 0 },
  { dr: 1, dc: 0 },
  { dr: 0, dc: -1 },
  { dr: 0, dc: 1 },
  { dr: -1, dc: -1 },
  { dr: -1, dc: 1 },
  { dr: 1, dc: -1 },
  { dr: 1, dc: 1 },
];

const MODE_LABELS = {
  local: "Local two-player",
  "engine-black": "Random engine as Black",
  "engine-white": "Random engine as White",
  "engine-both": "Random engine on both sides",
};

const SAMPLE_RECORDS = {
  "game_0001.cbr": "../rust_chessball/tournament_out/game_0001.cbr",
  "game_0002.cbr": "../rust_chessball/tournament_out/game_0002.cbr",
};

const boardGrid = document.querySelector("#board-grid");
const fileLabelsTop = document.querySelector("#file-labels-top");
const fileLabelsBottom = document.querySelector("#file-labels-bottom");
const rankLabels = document.querySelector("#rank-labels");
const turnLabel = document.querySelector("#turn-label");
const statusPill = document.querySelector("#status-pill");
const modeReadout = document.querySelector("#mode-readout");
const ballReadout = document.querySelector("#ball-readout");
const memoryReadout = document.querySelector("#memory-readout");
const selectionPanel = document.querySelector("#selection-panel");
const moveHistoryList = document.querySelector("#move-history");
const moveCount = document.querySelector("#move-count");
const modeSelect = document.querySelector("#mode-select");
const newGameBtn = document.querySelector("#new-game-btn");
const undoBtn = document.querySelector("#undo-btn");
const randomMoveBtn = document.querySelector("#random-move-btn");
const engineNowBtn = document.querySelector("#engine-now-btn");
const sampleSelect = document.querySelector("#sample-select");
const loadSampleBtn = document.querySelector("#load-sample-btn");
const cbrFileInput = document.querySelector("#cbr-file-input");
const replaySummary = document.querySelector("#replay-summary");
const replayResult = document.querySelector("#replay-result");
const replayStep = document.querySelector("#replay-step");
const replaySlider = document.querySelector("#replay-slider");
const replayStartBtn = document.querySelector("#replay-start-btn");
const replayPrevBtn = document.querySelector("#replay-prev-btn");
const replayNextBtn = document.querySelector("#replay-next-btn");
const replayEndBtn = document.querySelector("#replay-end-btn");
const replayMessage = document.querySelector("#replay-message");

const state = {
  board: createInitialBoard(),
  currentPlayer: WHITE,
  prevTackle: null,
  outcome: null,
  mode: "local",
  history: [],
  moveHistory: [],
  selected: null,
  legalMoves: [],
  lastMove: null,
  engineTimer: null,
  replay: null,
  replayMessage:
    "Load a Rust-generated CBR1 record to step through the game on the board.",
};

bootstrap();

function bootstrap() {
  renderAxisLabels();
  wireEvents();
  refreshLegalMoves();
  render();
  scheduleEngineTurnIfNeeded();
}

function wireEvents() {
  modeSelect.addEventListener("change", () => {
    state.mode = modeSelect.value;
    clearEngineTimer();
    render();
    scheduleEngineTurnIfNeeded();
  });

  newGameBtn.addEventListener("click", () => {
    startNewGame();
  });

  undoBtn.addEventListener("click", () => {
    undoMove();
  });

  randomMoveBtn.addEventListener("click", () => {
    playRandomMove();
  });

  engineNowBtn.addEventListener("click", () => {
    playRandomMove({ engineOnly: true, actor: "engine" });
  });

  loadSampleBtn.addEventListener("click", async () => {
    await loadSelectedSample();
  });

  cbrFileInput.addEventListener("change", async (event) => {
    const [file] = event.target.files || [];
    if (!file) {
      return;
    }
    try {
      const text = await file.text();
      loadReplayText(text, file.name);
    } catch (error) {
      setReplayMessage(`Failed to read ${file.name}: ${error.message}`);
      render();
    } finally {
      cbrFileInput.value = "";
    }
  });

  replaySlider.addEventListener("input", () => {
    if (!state.replay) {
      return;
    }
    setReplayIndex(Number(replaySlider.value));
  });

  replayStartBtn.addEventListener("click", () => {
    setReplayIndex(0);
  });

  replayPrevBtn.addEventListener("click", () => {
    if (state.replay) {
      setReplayIndex(state.replay.currentIndex - 1);
    }
  });

  replayNextBtn.addEventListener("click", () => {
    if (state.replay) {
      setReplayIndex(state.replay.currentIndex + 1);
    }
  });

  replayEndBtn.addEventListener("click", () => {
    if (state.replay) {
      setReplayIndex(state.replay.moves.length);
    }
  });
}

function renderAxisLabels() {
  const axis = ['<span></span>']
    .concat(FILES.map((file) => `<span>${file}</span>`))
    .join("");
  fileLabelsTop.innerHTML = axis;
  fileLabelsBottom.innerHTML = axis;
  rankLabels.innerHTML = RANKS.map((rank) => `<span>${rank}</span>`).join("");
}

function startNewGame() {
  clearEngineTimer();
  state.board = createInitialBoard();
  state.currentPlayer = WHITE;
  state.prevTackle = null;
  state.outcome = null;
  state.history = [];
  state.moveHistory = [];
  state.selected = null;
  state.lastMove = null;
  state.replay = null;
  setReplayMessage(
    "Load a Rust-generated CBR1 record to step through the game on the board.",
  );
  refreshLegalMoves();
  render();
  scheduleEngineTurnIfNeeded();
}

function undoMove() {
  if (isReplayMode()) {
    return;
  }

  clearEngineTimer();
  const snapshot = state.history.pop();
  if (!snapshot) {
    return;
  }

  state.board = cloneBoard(snapshot.board);
  state.currentPlayer = snapshot.currentPlayer;
  state.prevTackle = clonePrevTackle(snapshot.prevTackle);
  state.outcome = snapshot.outcome ? { ...snapshot.outcome } : null;
  state.moveHistory = snapshot.moveHistory.map((entry) => ({ ...entry }));
  state.lastMove = snapshot.lastMove ? { ...snapshot.lastMove } : null;
  state.selected = null;
  refreshLegalMoves();
  render();
  scheduleEngineTurnIfNeeded();
}

function playRandomMove(options = {}) {
  if (isReplayMode()) {
    return;
  }
  if (state.outcome || state.legalMoves.length === 0) {
    render();
    return;
  }
  if (options.engineOnly && !isEngineTurn(state.currentPlayer)) {
    return;
  }

  const move = state.legalMoves[Math.floor(Math.random() * state.legalMoves.length)];
  commitMove(move, { actor: options.actor || "random" });
}

function scheduleEngineTurnIfNeeded() {
  clearEngineTimer();
  if (
    isReplayMode() ||
    !isEngineTurn(state.currentPlayer) ||
    state.outcome ||
    state.legalMoves.length === 0
  ) {
    return;
  }

  state.engineTimer = window.setTimeout(() => {
    playRandomMove({ actor: "engine" });
  }, 450);
}

function clearEngineTimer() {
  if (state.engineTimer !== null) {
    window.clearTimeout(state.engineTimer);
    state.engineTimer = null;
  }
}

function createInitialBoard() {
  const board = emptyBoard();
  place(board, { r: 0, c: 1 }, "WD");
  place(board, { r: 0, c: 3 }, "WD");
  place(board, { r: 0, c: 5 }, "WD");
  place(board, { r: 1, c: 2 }, "WA");
  place(board, { r: 1, c: 4 }, "WA");
  place(board, { r: 2, c: 3 }, BALL);
  place(board, { r: 4, c: 2 }, "BA");
  place(board, { r: 4, c: 4 }, "BA");
  place(board, { r: 5, c: 1 }, "BD");
  place(board, { r: 5, c: 3 }, "BD");
  place(board, { r: 5, c: 5 }, "BD");
  return board;
}

function emptyBoard() {
  return Array.from({ length: ROWS }, () => Array(COLS).fill(null));
}

function cloneBoard(board) {
  return board.map((row) => row.slice());
}

function clonePrevTackle(prevTackle) {
  if (!prevTackle) {
    return null;
  }
  return {
    from: { ...prevTackle.from },
    to: { ...prevTackle.to },
  };
}

function snapshotState() {
  return {
    board: cloneBoard(state.board),
    currentPlayer: state.currentPlayer,
    prevTackle: clonePrevTackle(state.prevTackle),
    outcome: state.outcome ? { ...state.outcome } : null,
    moveHistory: state.moveHistory.map((entry) => ({ ...entry })),
    lastMove: state.lastMove ? { ...state.lastMove } : null,
  };
}

function onBoard(coord) {
  return coord.r >= 0 && coord.r < ROWS && coord.c >= 0 && coord.c < COLS;
}

function add(coord, delta) {
  const next = { r: coord.r + delta.dr, c: coord.c + delta.dc };
  return onBoard(next) ? next : null;
}

function place(board, coord, piece) {
  board[coord.r][coord.c] = piece;
}

function at(board, coord) {
  return board[coord.r][coord.c];
}

function isForbiddenBallDestination(coord) {
  return (coord.c === 0 || coord.c === COLS - 1) && coord.r > 0 && coord.r + 1 < ROWS;
}

function pieceOwner(piece) {
  return piece ? piece[0] : null;
}

function pieceType(piece) {
  return piece ? piece[1] : null;
}

function otherPlayer(player) {
  return player === WHITE ? BLACK : WHITE;
}

function findBall(board) {
  for (let r = 0; r < ROWS; r += 1) {
    for (let c = 0; c < COLS; c += 1) {
      if (board[r][c] === BALL) {
        return { r, c };
      }
    }
  }
  return null;
}

function goalRow(player) {
  return player === WHITE ? ROWS - 1 : 0;
}

function winnerForBall(ball) {
  if (!ball) {
    return null;
  }
  if (ball.r === goalRow(WHITE)) {
    return WHITE;
  }
  if (ball.r === goalRow(BLACK)) {
    return BLACK;
  }
  return null;
}

function refreshLegalMoves() {
  if (isReplayMode()) {
    state.selected = null;
    state.legalMoves = [];
    return;
  }

  state.selected = null;
  state.legalMoves = generateMoves(state.board, state.currentPlayer, state.prevTackle);
  if (!state.outcome && state.legalMoves.length === 0) {
    state.outcome = {
      type: "stalled",
      message:
        "No legal moves remain for the side to move. The current rules spec defines no winner for this case.",
    };
  }
}

function generateMoves(board, player, prevTackle) {
  const moves = [];

  for (let r = 0; r < ROWS; r += 1) {
    for (let c = 0; c < COLS; c += 1) {
      const from = { r, c };
      const piece = at(board, from);
      if (!piece || pieceOwner(piece) !== player) {
        continue;
      }

      for (const delta of DIRECTIONS) {
        const adjacent = add(from, delta);
        if (!adjacent) {
          continue;
        }

        const adjacentPiece = at(board, adjacent);
        if (!adjacentPiece) {
          moves.push(buildSimpleMove(board, piece, from, adjacent));
          continue;
        }

        if (adjacentPiece === BALL) {
          const ballTo = add(adjacent, delta);
          if (ballTo && !at(board, ballTo) && !isForbiddenBallDestination(ballTo)) {
            moves.push(buildBallPushMove(board, piece, from, adjacent, ballTo));
          }
          if (
            pieceType(piece) === "A" &&
            ballTo &&
            !at(board, ballTo) &&
            !isImmediateRevengeJump(prevTackle, from, adjacent)
          ) {
            moves.push(buildAttackerJumpMove(board, piece, from, adjacent, ballTo));
          }
          continue;
        }

        const landing = add(adjacent, delta);
        if (!landing || at(board, landing)) {
          continue;
        }

        if (
          pieceType(piece) === "A" &&
          !isImmediateRevengeJump(prevTackle, from, adjacent)
        ) {
          moves.push(buildAttackerJumpMove(board, piece, from, adjacent, landing));
        }

        if (
          pieceType(piece) === "D" &&
          pieceOwner(adjacentPiece) === otherPlayer(player) &&
          !isImmediateRevengeTackle(prevTackle, from, adjacent)
        ) {
          moves.push(buildDefenderTackleMove(board, piece, from, adjacent, landing));
        }
      }
    }
  }

  return moves;
}

function isImmediateRevengeJump(prevTackle, from, jumpedOver) {
  return Boolean(
    prevTackle &&
      sameCoord(prevTackle.from, jumpedOver) &&
      sameCoord(prevTackle.to, from),
  );
}

function isImmediateRevengeTackle(prevTackle, from, to) {
  return Boolean(
    prevTackle && sameCoord(prevTackle.from, to) && sameCoord(prevTackle.to, from),
  );
}

function buildSimpleMove(board, piece, from, to) {
  const nextBoard = cloneBoard(board);
  place(nextBoard, from, null);
  place(nextBoard, to, piece);

  return {
    type: "simple",
    from,
    to,
    piece,
    nextBoard,
    nextPrevTackle: null,
    notation: `${coordToNotation(from)}-${coordToNotation(to)}`,
    detail: "simple move",
  };
}

function buildBallPushMove(board, piece, from, to, ballTo) {
  const nextBoard = cloneBoard(board);
  place(nextBoard, from, null);
  place(nextBoard, to, piece);
  place(nextBoard, ballTo, BALL);

  return {
    type: "push",
    from,
    to,
    piece,
    ballTo,
    nextBoard,
    nextPrevTackle: null,
    notation: `${coordToNotation(from)}x${coordToNotation(to)}`,
    detail: `push ball to ${coordToNotation(ballTo)}`,
  };
}

function buildAttackerJumpMove(board, piece, from, jumpedOver, to) {
  const nextBoard = cloneBoard(board);
  place(nextBoard, from, null);
  place(nextBoard, to, piece);

  return {
    type: "jump",
    from,
    to,
    piece,
    jumpedOver,
    nextBoard,
    nextPrevTackle: null,
    notation: `${coordToNotation(from)}>${coordToNotation(to)}`,
    detail: `jump over ${coordToNotation(jumpedOver)}`,
  };
}

function buildDefenderTackleMove(board, piece, from, to, pushedTo) {
  const nextBoard = cloneBoard(board);
  const pushedPiece = at(board, to);
  place(nextBoard, from, null);
  place(nextBoard, pushedTo, pushedPiece);
  place(nextBoard, to, piece);

  return {
    type: "tackle",
    from,
    to,
    piece,
    pushedTo,
    nextBoard,
    nextPrevTackle: {
      from: { ...to },
      to: { ...pushedTo },
    },
    notation: `${coordToNotation(from)}!${coordToNotation(to)}`,
    detail: `tackle to ${coordToNotation(to)}, push victim to ${coordToNotation(pushedTo)}`,
  };
}

function sameCoord(a, b) {
  return a && b && a.r === b.r && a.c === b.c;
}

function coordKey(coord) {
  return `${coord.r},${coord.c}`;
}

function coordToNotation(coord) {
  return `${FILES[coord.c]}${ROWS - coord.r}`;
}

function parseSquareNotation(text) {
  if (!/^[a-g][1-6]$/i.test(text)) {
    throw new Error(`Invalid square '${text}'`);
  }
  const file = text[0].toLowerCase();
  const rank = Number(text[1]);
  return {
    r: ROWS - rank,
    c: file.charCodeAt(0) - 97,
  };
}

function playerName(player) {
  return player === WHITE ? "White" : "Black";
}

function playerFromChar(text) {
  if (text === WHITE || text === BLACK) {
    return text;
  }
  throw new Error(`Invalid player '${text}'`);
}

function getMovesForSelection() {
  if (!state.selected || isReplayMode()) {
    return [];
  }
  return state.legalMoves.filter((move) => sameCoord(move.from, state.selected));
}

function getMovableSquares() {
  if (isReplayMode()) {
    return new Set();
  }
  return new Set(state.legalMoves.map((move) => coordKey(move.from)));
}

function getTargetMovesBySquare() {
  const bySquare = new Map();
  for (const move of getMovesForSelection()) {
    bySquare.set(coordKey(move.to), move);
  }
  return bySquare;
}

function handleCellClick(coord) {
  if (isReplayMode() || state.outcome) {
    return;
  }

  const piece = at(state.board, coord);
  const selectionMoves = getTargetMovesBySquare();
  const moveFromSelection = selectionMoves.get(coordKey(coord));
  if (moveFromSelection) {
    commitMove(moveFromSelection, { actor: "human" });
    return;
  }

  if (piece && pieceOwner(piece) === state.currentPlayer) {
    const pieceHasMoves = state.legalMoves.some((move) => sameCoord(move.from, coord));
    state.selected = pieceHasMoves ? coord : null;
  } else {
    state.selected = null;
  }

  render();
}

function commitMove(move, meta = {}) {
  clearEngineTimer();
  state.history.push(snapshotState());
  state.board = cloneBoard(move.nextBoard);
  state.prevTackle = clonePrevTackle(move.nextPrevTackle);
  state.selected = null;
  state.lastMove = {
    from: { ...move.from },
    to: { ...move.to },
  };

  const mover = state.currentPlayer;
  const ball = findBall(state.board);
  const winner = winnerForBall(ball);

  state.moveHistory.push({
    ply: state.moveHistory.length + 1,
    player: playerName(mover),
    notation: move.notation,
    detail: move.detail,
    actor: meta.actor || "human",
  });

  if (winner) {
    state.outcome = {
      type: "win",
      winner,
      message: `${playerName(winner)} wins because the ball reached ${coordToNotation(ball)}.`,
    };
  } else {
    state.currentPlayer = otherPlayer(state.currentPlayer);
    state.outcome = null;
  }

  refreshLegalMoves();
  render();
  scheduleEngineTurnIfNeeded();
}

function isEngineTurn(player) {
  if (state.mode === "engine-both") {
    return true;
  }
  if (state.mode === "engine-white") {
    return player === WHITE;
  }
  if (state.mode === "engine-black") {
    return player === BLACK;
  }
  return false;
}

function isReplayMode() {
  return state.replay !== null;
}

async function loadSelectedSample() {
  const name = sampleSelect.value;
  const path = SAMPLE_RECORDS[name];
  if (!path) {
    setReplayMessage(`Unknown sample '${name}'`);
    render();
    return;
  }

  try {
    const response = await fetch(path);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    const text = await response.text();
    loadReplayText(text, name);
  } catch (error) {
    setReplayMessage(
      `Failed to load ${name}. Serve the repo with a local HTTP server and try again. (${error.message})`,
    );
    render();
  }
}

function loadReplayText(text, label) {
  try {
    clearEngineTimer();
    const record = parseCbrRecord(text);
    const replay = buildReplayState(record, label);
    state.replay = replay;
    state.history = [];
    state.moveHistory = [];
    state.selected = null;
    setReplayMessage(`Loaded ${label} successfully.`);
    applyReplayFrame(0);
  } catch (error) {
    setReplayMessage(`Replay load failed: ${error.message}`);
    render();
  }
}

function setReplayMessage(message) {
  state.replayMessage = message;
}

function parseCbrRecord(text) {
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  let index = 0;

  const readLine = () => {
    const line = lines[index];
    if (line === undefined) {
      throw new Error("Unexpected end of CBR file");
    }
    index += 1;
    return line;
  };

  const expectExact = (expected) => {
    const line = readLine().trim();
    if (line !== expected) {
      throw new Error(`Expected '${expected}', got '${line}'`);
    }
  };

  const expectPrefix = (prefix) => {
    const line = readLine();
    if (!line.startsWith(prefix)) {
      throw new Error(`Expected line starting with '${prefix}'`);
    }
    return line.slice(prefix.length).trim();
  };

  const readBoard = () => {
    const boardLines = [];
    for (let count = 0; count < ROWS; count += 1) {
      boardLines.push(readLine().trim());
    }
    return parseBoardLines(boardLines);
  };

  expectExact("CBR1");
  const whiteLabel = expectPrefix("White: ");
  const blackLabel = expectPrefix("Black: ");
  const initialToMove = playerFromChar(expectPrefix("Initial-To-Move: "));
  expectExact("Initial-Board:");
  const initialBoard = readBoard();
  expectExact("Moves:");

  const moves = [];
  while (index < lines.length) {
    const line = lines[index];
    if (line === undefined) {
      break;
    }
    if (line.trim() === "") {
      index += 1;
      continue;
    }
    if (line.startsWith("Result: ")) {
      break;
    }
    moves.push(parseCbrMoveLine(line.trim()));
    index += 1;
  }

  const result = expectPrefix("Result: ");
  const termination = expectPrefix("Termination: ");
  expectExact("Final-Board:");
  const finalBoard = readBoard();

  return {
    whiteLabel,
    blackLabel,
    initialToMove,
    initialBoard,
    moves,
    result,
    termination,
    finalBoard,
  };
}

function parseBoardLines(lines) {
  if (lines.length !== ROWS) {
    throw new Error(`Expected ${ROWS} board rows, got ${lines.length}`);
  }

  const board = emptyBoard();
  for (let r = 0; r < ROWS; r += 1) {
    const tokens = lines[r].split(/\s+/);
    if (tokens.length !== COLS) {
      throw new Error(`Invalid board row '${lines[r]}'`);
    }
    for (let c = 0; c < COLS; c += 1) {
      const token = tokens[c];
      if (token === "--") {
        continue;
      }
      if (!["WA", "WD", "BA", "BD", "NB"].includes(token)) {
        throw new Error(`Invalid board token '${token}'`);
      }
      place(board, { r, c }, token);
    }
  }

  return board;
}

function parseCbrMoveLine(line) {
  const tokens = line.split(/\s+/);
  if (tokens.length < 4 || !tokens[0].endsWith(".")) {
    throw new Error(`Invalid move line '${line}'`);
  }

  const player = playerFromChar(tokens[1]);
  const move = parseRecordedMoveNotation(tokens[2]);
  const metadata = {
    source: null,
    score: null,
    nodes: null,
  };

  for (const token of tokens.slice(3)) {
    if (token.startsWith("source=")) {
      metadata.source = token.slice("source=".length);
    } else if (token.startsWith("score=")) {
      metadata.score = Number(token.slice("score=".length));
    } else if (token.startsWith("nodes=")) {
      metadata.nodes = Number(token.slice("nodes=".length));
    }
  }

  if (!metadata.source) {
    throw new Error(`Missing move source in '${line}'`);
  }

  return {
    player,
    ...move,
    source: metadata.source,
    score: Number.isFinite(metadata.score) ? metadata.score : null,
    nodes: Number.isFinite(metadata.nodes) ? metadata.nodes : null,
  };
}

function parseRecordedMoveNotation(text) {
  let body = text;
  let type = "simple";
  let suffixCoord = null;

  for (const [marker, nextType] of [
    ["@", "push"],
    ["^", "jump"],
    ["!", "tackle"],
  ]) {
    if (text.includes(marker)) {
      const parts = text.split(marker);
      if (parts.length !== 2) {
        throw new Error(`Invalid move notation '${text}'`);
      }
      [body] = parts;
      type = nextType;
      suffixCoord = parseSquareNotation(parts[1]);
      break;
    }
  }

  const [fromText, toText] = body.split("-");
  if (!fromText || !toText) {
    throw new Error(`Invalid move notation '${text}'`);
  }

  const move = {
    notation: text,
    type,
    from: parseSquareNotation(fromText),
    to: parseSquareNotation(toText),
  };

  if (type === "push") {
    move.ballTo = suffixCoord;
  } else if (type === "jump") {
    move.jumpedOver = suffixCoord;
  } else if (type === "tackle") {
    move.pushedTo = suffixCoord;
  }

  return move;
}

function buildReplayState(record, label) {
  const positions = [
    {
      board: cloneBoard(record.initialBoard),
      toMove: record.initialToMove,
      prevTackle: null,
      lastMove: null,
    },
  ];

  let current = positions[0];
  for (const ply of record.moves) {
    if (ply.player !== current.toMove) {
      throw new Error(
        `Move player ${ply.player} does not match side to move ${current.toMove}`,
      );
    }

    const legalMoves = generateMoves(current.board, current.toMove, current.prevTackle);
    const resolved = legalMoves.find((candidate) => moveMatchesRecord(candidate, ply));
    if (!resolved) {
      throw new Error(`Illegal or unsupported record move '${ply.notation}'`);
    }

    ply.detail = resolved.detail;

    current = {
      board: cloneBoard(resolved.nextBoard),
      toMove: otherPlayer(current.toMove),
      prevTackle: clonePrevTackle(resolved.nextPrevTackle),
      lastMove: {
        from: { ...resolved.from },
        to: { ...resolved.to },
      },
    };
    positions.push(current);
  }

  if (!boardsEqual(positions[positions.length - 1].board, record.finalBoard)) {
    throw new Error("Final board does not match replayed moves");
  }

  return {
    label,
    ...record,
    positions,
    currentIndex: 0,
  };
}

function moveMatchesRecord(candidate, recordMove) {
  if (
    candidate.type !== recordMove.type ||
    !sameCoord(candidate.from, recordMove.from) ||
    !sameCoord(candidate.to, recordMove.to)
  ) {
    return false;
  }

  if (candidate.type === "push") {
    return sameCoord(candidate.ballTo, recordMove.ballTo);
  }
  if (candidate.type === "jump") {
    return sameCoord(candidate.jumpedOver, recordMove.jumpedOver);
  }
  if (candidate.type === "tackle") {
    return sameCoord(candidate.pushedTo, recordMove.pushedTo);
  }
  return true;
}

function boardsEqual(left, right) {
  for (let r = 0; r < ROWS; r += 1) {
    for (let c = 0; c < COLS; c += 1) {
      if (left[r][c] !== right[r][c]) {
        return false;
      }
    }
  }
  return true;
}

function setReplayIndex(index) {
  if (!state.replay) {
    return;
  }
  const clamped = Math.max(0, Math.min(index, state.replay.moves.length));
  applyReplayFrame(clamped);
}

function applyReplayFrame(index) {
  if (!state.replay) {
    return;
  }

  const frame = state.replay.positions[index];
  state.replay.currentIndex = index;
  state.board = cloneBoard(frame.board);
  state.currentPlayer = frame.toMove;
  state.prevTackle = clonePrevTackle(frame.prevTackle);
  state.lastMove = frame.lastMove ? { ...frame.lastMove } : null;
  state.outcome = null;
  state.selected = null;
  state.legalMoves = [];
  render();
}

function render() {
  renderBoard();
  renderStatus();
  renderSelectionPanel();
  renderMoveHistory();
  renderReplayControls();
  renderControls();
}

function renderBoard() {
  const movableSquares = getMovableSquares();
  const targetMoves = getTargetMovesBySquare();
  boardGrid.innerHTML = "";

  for (let r = 0; r < ROWS; r += 1) {
    for (let c = 0; c < COLS; c += 1) {
      const coord = { r, c };
      const piece = at(state.board, coord);
      const button = document.createElement("button");
      button.type = "button";
      button.className = "cell";
      button.setAttribute("aria-label", buildAriaLabel(coord, piece));

      if (isForbiddenBallDestination(coord)) {
        button.classList.add("forbidden-column");
      }
      if (state.selected && sameCoord(state.selected, coord)) {
        button.classList.add("selected");
      }
      if (movableSquares.has(coordKey(coord)) && !state.outcome) {
        button.classList.add("movable");
      }
      if (targetMoves.has(coordKey(coord))) {
        const move = targetMoves.get(coordKey(coord));
        button.classList.add("legal-target", `target-${move.type}`);
      }
      if (state.lastMove && sameCoord(state.lastMove.from, coord)) {
        button.classList.add("last-from");
      }
      if (state.lastMove && sameCoord(state.lastMove.to, coord)) {
        button.classList.add("last-to");
      }

      button.addEventListener("click", () => {
        handleCellClick(coord);
      });

      if (piece) {
        button.innerHTML = renderPiece(piece);
      }

      boardGrid.appendChild(button);
    }
  }
}

function buildAriaLabel(coord, piece) {
  const location = coordToNotation(coord);
  if (!piece) {
    return `Empty square ${location}`;
  }
  if (piece === BALL) {
    return `Ball on ${location}`;
  }
  const owner = pieceOwner(piece) === WHITE ? "White" : "Black";
  const type = pieceType(piece) === "A" ? "attacker" : "defender";
  return `${owner} ${type} on ${location}`;
}

function renderPiece(piece) {
  if (piece === BALL) {
    return `<span class="token token-ball">●</span><span class="cell-code">ball</span>`;
  }

  const ownerClass = pieceOwner(piece) === WHITE ? "token-white" : "token-black";
  const kindClass = pieceType(piece) === "A" ? "attacker" : "defender";
  return `<span class="token ${ownerClass} ${kindClass}">${pieceType(piece)}</span><span class="cell-code">${piece}</span>`;
}

function renderStatus() {
  const ball = findBall(state.board);

  if (isReplayMode()) {
    const atEnd = state.replay.currentIndex === state.replay.moves.length;
    turnLabel.textContent = atEnd
      ? `Replay final · ${state.replay.currentIndex}/${state.replay.moves.length} plies`
      : `Replay ply ${state.replay.currentIndex}/${state.replay.moves.length}`;
    statusPill.className = "status-pill";
    statusPill.textContent = atEnd ? "Replay final" : "Replay";
    modeReadout.textContent = `Replay: ${state.replay.whiteLabel} vs ${state.replay.blackLabel}`;
  } else {
    turnLabel.textContent =
      state.outcome?.type === "win"
        ? `${playerName(state.outcome.winner)} has won`
        : `${playerName(state.currentPlayer)} to move`;
    statusPill.className = "status-pill";
    if (!state.outcome) {
      statusPill.textContent = "Live game";
    } else if (state.outcome.type === "win") {
      statusPill.textContent = "Goal reached";
      statusPill.classList.add("win");
    } else {
      statusPill.textContent = "Stalled";
      statusPill.classList.add("stalled");
    }
    modeReadout.textContent = `Mode: ${MODE_LABELS[state.mode]}`;
  }

  ballReadout.textContent = ball ? `Ball: ${coordToNotation(ball)}` : "Ball: missing";
  memoryReadout.textContent = state.prevTackle
    ? `Last tackle: ${coordToNotation(state.prevTackle.from)} -> ${coordToNotation(state.prevTackle.to)}`
    : "Last tackle: none";
}

function renderSelectionPanel() {
  if (isReplayMode()) {
    const { currentIndex, moves, whiteLabel, blackLabel, result, termination } = state.replay;
    if (currentIndex === 0) {
      selectionPanel.innerHTML = `
        <strong>Initial position</strong>
        <p>${escapeHtml(whiteLabel)} vs ${escapeHtml(blackLabel)}</p>
        <p>Result: ${escapeHtml(result)} · Termination: ${escapeHtml(termination)}</p>
      `;
      return;
    }

    const move = moves[currentIndex - 1];
    const stats = [];
    if (move.source) {
      stats.push(`source=${escapeHtml(move.source)}`);
    }
    if (move.score !== null) {
      stats.push(`score=${escapeHtml(String(move.score))}`);
    }
    if (move.nodes !== null) {
      stats.push(`nodes=${escapeHtml(String(move.nodes))}`);
    }

    selectionPanel.innerHTML = `
      <strong>Ply ${currentIndex}: ${escapeHtml(playerName(move.player))} ${escapeHtml(move.notation)}</strong>
      <p>${escapeHtml(move.detail || "recorded move")}</p>
      <p>${stats.join(" · ")}</p>
      <p>Result: ${escapeHtml(result)} · Termination: ${escapeHtml(termination)}</p>
    `;
    return;
  }

  if (state.outcome) {
    selectionPanel.innerHTML = `<strong>${escapeHtml(state.outcome.message)}</strong>`;
    return;
  }

  if (state.legalMoves.length === 0) {
    selectionPanel.innerHTML =
      "<strong>No legal moves available.</strong> Use Undo or start a new game.";
    return;
  }

  if (!state.selected) {
    selectionPanel.innerHTML =
      "<p>Select one of the highlighted pieces, then click a legal target square.</p>";
    return;
  }

  const moves = getMovesForSelection();
  const piece = at(state.board, state.selected);
  const pieceLabel =
    piece === BALL
      ? "Ball"
      : `${playerName(pieceOwner(piece))} ${
          pieceType(piece) === "A" ? "attacker" : "defender"
        }`;
  const items = moves
    .map(
      (move) =>
        `<li><strong>${coordToNotation(move.to)}</strong> <span>${escapeHtml(move.detail)}</span></li>`,
    )
    .join("");
  selectionPanel.innerHTML = `
    <strong>${escapeHtml(pieceLabel)} at ${coordToNotation(state.selected)}</strong>
    <ul>${items}</ul>
  `;
}

function renderMoveHistory() {
  if (isReplayMode()) {
    moveCount.textContent = `${state.replay.moves.length} pl${state.replay.moves.length === 1 ? "y" : "ies"}`;
    if (state.replay.moves.length === 0) {
      moveHistoryList.innerHTML = "<li>Empty record.</li>";
      return;
    }

    moveHistoryList.innerHTML = state.replay.moves
      .map((entry, index) => {
        const meta = [entry.detail || "recorded move", `source=${entry.source}`];
        if (entry.score !== null) {
          meta.push(`score=${entry.score}`);
        }
        if (entry.nodes !== null) {
          meta.push(`nodes=${entry.nodes}`);
        }
        const activeClass = state.replay.currentIndex === index + 1 ? "active" : "";
        return `
          <li class="${activeClass}">
            <strong>${index + 1}. ${escapeHtml(playerName(entry.player))}: ${escapeHtml(entry.notation)}</strong>
            <div class="meta">${escapeHtml(meta.join(" · "))}</div>
          </li>
        `;
      })
      .join("");
    return;
  }

  moveCount.textContent = `${state.moveHistory.length} pl${state.moveHistory.length === 1 ? "y" : "ies"}`;

  if (state.moveHistory.length === 0) {
    moveHistoryList.innerHTML = "<li>Game start.</li>";
    return;
  }

  moveHistoryList.innerHTML = state.moveHistory
    .map(
      (entry) => `
        <li>
          <strong>${entry.ply}. ${escapeHtml(entry.player)}: ${escapeHtml(entry.notation)}</strong>
          <div class="meta">${escapeHtml(entry.detail)}${entry.actor === "engine" ? " · random engine" : ""}</div>
        </li>
      `,
    )
    .join("");
}

function renderReplayControls() {
  if (!state.replay) {
    replaySummary.textContent = "No replay loaded";
    replayResult.textContent = "Result: -";
    replayStep.textContent = "Step: 0 / 0";
    replaySlider.min = "0";
    replaySlider.max = "0";
    replaySlider.value = "0";
    replaySlider.disabled = true;
    replayStartBtn.disabled = true;
    replayPrevBtn.disabled = true;
    replayNextBtn.disabled = true;
    replayEndBtn.disabled = true;
    replayMessage.textContent = state.replayMessage;
    return;
  }

  replaySummary.textContent = `${state.replay.label} · ${state.replay.whiteLabel} vs ${state.replay.blackLabel}`;
  replayResult.textContent = `Result: ${state.replay.result} · ${state.replay.termination}`;
  replayStep.textContent = `Step: ${state.replay.currentIndex} / ${state.replay.moves.length}`;
  replaySlider.min = "0";
  replaySlider.max = String(state.replay.moves.length);
  replaySlider.value = String(state.replay.currentIndex);
  replaySlider.disabled = false;
  replayStartBtn.disabled = state.replay.currentIndex === 0;
  replayPrevBtn.disabled = state.replay.currentIndex === 0;
  replayNextBtn.disabled = state.replay.currentIndex === state.replay.moves.length;
  replayEndBtn.disabled = state.replay.currentIndex === state.replay.moves.length;
  replayMessage.textContent = state.replayMessage;
}

function renderControls() {
  const replayMode = isReplayMode();
  undoBtn.disabled = replayMode || state.history.length === 0;
  randomMoveBtn.disabled = replayMode || Boolean(state.outcome || state.legalMoves.length === 0);
  engineNowBtn.disabled = replayMode || Boolean(
    state.outcome || state.legalMoves.length === 0 || !isEngineTurn(state.currentPlayer),
  );
}

function escapeHtml(text) {
  return String(text)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}
