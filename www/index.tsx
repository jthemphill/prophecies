import React from "react";
import ReactDOM from "react-dom";
import * as wasm from "prophecies";

const CELL_SIZE_PX = 100;
const GRID_COLOR = "#999999";

const HUMAN_PLAYER = 0;
const BOT_PLAYER = 1;

const HUMAN_COLOR = "#d63b3b";
const BOT_COLOR = "#3bd6d0";

const MIN_PLAYOUTS_MS = 256;
const PONDER_MS = 128;
const PONDER_INTERVAL = 1024;

const legalActionRegex = new RegExp("^X|[0-9]+$");

type CellProps = {
    guess?: number,
    player?: number,
    onBlur: ((row: number, col: number, e: FocusEvent) => void),
    onChange: ((row: number, col: number, e: Event) => void),
    row: number,
    col: number,
};
type CellState = {
    content: string,
};
class Cell extends React.PureComponent<CellProps, CellState> {

    constructor(props: CellProps, context?: any) {
        super(props, context);
        this.state = {
            content: "",
        };
    }

    render() {
        return <td>
            <input
                disabled={this.props.guess !== null}
                inputMode="numeric"
                onBlur={this.onBlur.bind(this)}
                onChange={this.onChange.bind(this)}
                style={{
                    color: this.getColor(),
                }}
                value={this.getContent()}
            ></input>
        </td>;
    }

    getColor() {
        if (this.props.player == HUMAN_PLAYER || this.state.content.length > 0) {
            return HUMAN_COLOR;
        } else if (this.props.player == BOT_PLAYER) {
            return BOT_COLOR;
        }
        return null;
    }

    getContent() {
        if (this.props.guess == null) {
            return this.state.content;
        }
        if (this.props.guess === 0) {
            return "X";
        }
        return this.props.guess;
    }

    onBlur(e: FocusEvent) {
        if (legalActionRegex.test(this.state.content)) {
            try {
                this.props.onBlur(this.props.row, this.props.col, e);
            } catch (err) {
                console.log(err);
            }
        }
        this.setState({ content: "" });
    }

    onChange(e: Event) {
        this.setState({ content: (e.currentTarget as HTMLInputElement).value });
        return this.props.onChange(this.props.row, this.props.col, e);
    }
}

type GridProps = {
    nrows: number,
    ncols: number,
};
type GridState = {
    activePlayer?: number,
    grid: { guess?: number, player?: number }[][],
    scores: number[],
    winProb?: number,
};
class Grid extends React.PureComponent<GridProps, GridState> {

    bot: wasm.WasmBot;
    shouldPonder: boolean;

    constructor(props: GridProps, context: any) {
        super(props, context);
        this.state = this.getGameState();
    }

    componentDidMount() {
        this.bot = wasm.WasmBot.new(
            this.props.nrows,
            this.props.ncols,
            HUMAN_PLAYER,
        );
        this.setState(this.getGameState());
        this.shouldPonder = true;
        this.ponder();
    }

    componentWillUnmount() {
        this.shouldPonder = false;
        this.bot.free();
    }

    getGameState(): GridState {
        let grid = [];
        for (let r = 0; r < this.props.nrows; ++r) {
            let row = [];
            for (let c = 0; c < this.props.ncols; ++c) {
                if (this.bot) {
                    const cell = this.bot.get_cell(r, c);
                    row.push({
                        guess: cell.guess !== undefined ? cell.guess : null,
                        player: cell.player !== undefined ? cell.player : null,
                    });
                } else {
                    row.push({
                        guess: null,
                        player: null,
                    });
                }
            }
            grid.push(row);
        }
        return {
            activePlayer: this.bot ? this.bot.get_active_player() : HUMAN_PLAYER,
            winProb: this.getWinProb(),
            scores: this.bot ? [...this.bot.get_scores()] : [0, 0],
            grid,
        };
    }

    render() {
        if (this.state.activePlayer === BOT_PLAYER) {
            window.setTimeout(this.takeBotAction.bind(this), 0);
        }
        const rows = [];
        for (let r = 0; r < this.props.nrows; r++) {
            const row = [];
            for (let c = 0; c < this.props.ncols; c++) {
                const { guess, player } = this.state.grid[r][c];
                row.push(
                    <Cell
                        col={c}
                        guess={guess}
                        key={`${r},${c}`}
                        onBlur={this.onCellInputBlur.bind(this)}
                        onChange={this.onCellInputChange.bind(this)}
                        player={player}
                        row={r}
                    ></Cell>
                );
            }
            rows.push(<tr key={r}>{row}</tr>);
        }
        let cpuProb = null;
        if (this.state.winProb != null) {
            cpuProb = <div className="prediction">
                The CPU thinks you have
                a <span style={{ color: HUMAN_COLOR }}>{formatPercent(this.state.winProb)}</span> chance
                of winning.
            </div>;
        }
        return <div className="prophecies-game">
            <table><tbody>
                {rows}
            </tbody></table>
            <div className="scores">
                <div>Scores:</div>
                <div>
                    <span style={{ color: HUMAN_COLOR }}>{this.state.scores[0]}</span>
                    <span> - </span>
                    <span style={{ color: BOT_COLOR }}>{this.state.scores[1]}</span>
                </div>
                {cpuProb}
            </div>
        </div>;
    }

    onCellInputBlur(row: number, col: number, e: FocusEvent) {
        // Already made a move in this cell
        if (this.state.grid[row][col].player !== null) {
            return;
        }
        const cellValue = (e.currentTarget as HTMLInputElement).value;
        if (!cellValue || !cellValue.length || !legalActionRegex.test(cellValue)) {
            return;
        }
        const guess = cellValue === "X" ? 0 : parseInt(cellValue, 10);
        this.takeAction({ row, col, guess });
    }

    onCellInputChange(row: number, col: number, e: Event) {
        const target = e.target as HTMLInputElement;
        if (!legalActionRegex.test(target.value)) {
            console.log(`${target.value} failed regex`);
            return;
        }
    }

    ponder() {
        if (!this.shouldPonder) {
            return;
        }
        if (this.bot.is_finished()) {
            this.shouldPonder = false;
            return;
        }
        const t0 = performance.now();
        let nplayouts = 0;
        while (performance.now() - t0 < PONDER_MS) {
            this.bot.playout();
            ++nplayouts;
        }
        const tf = performance.now();
        console.log(`${nplayouts} playouts in ${tf - t0} ms`);
        this.setState({ winProb: this.getWinProb() });
        window.setTimeout(this.ponder.bind(this), PONDER_INTERVAL);
    }

    getWinProb() {
        if (!this.bot) {
            return null;
        }
        if (this.bot.get_active_player() !== HUMAN_PLAYER) {
            return null;
        }
        const edge = this.bot.get_best_action();
        if (edge == null) {
            return null;
        }
        return (1 + edge.score / Number(edge.visits)) / 2;
    }

    takeAction(action: { row: number, col: number, guess: number }) {
        if (!action) {
            return;
        }
        // Throws IllegalMoveError if move is illegal
        this.bot.place(action.row, action.col, action.guess);
        this.setState(this.getGameState());
    }

    takeBotAction() {
        const t0 = performance.now();
        while (performance.now() - t0 < MIN_PLAYOUTS_MS) {
            this.bot.playout();
        }
        const edge = this.bot.get_best_action();
        if (edge == null) {
            this.shouldPonder = false;
            return;
        }
        this.takeAction(edge.action);
    }
}

function formatPercent(fraction: number) {
    const rounded = new Intl.NumberFormat(
        'en-US',
        { maximumSignificantDigits: 4 }
    ).format(fraction * 100);
    return `${rounded}%`;
}

ReactDOM.render(
    <Grid nrows={4} ncols={4} ></ Grid >,
    document.getElementById("prophecies-grid-container"),
);