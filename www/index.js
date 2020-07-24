import React from "react";
import ReactDOM from "react-dom";
import * as wasm from "prophecies";

const CELL_SIZE_PX = 100;
const GRID_COLOR = "#999999";

const HUMAN_PLAYER = 0;
const BOT_PLAYER = 1;

const HUMAN_COLOR = "#d63b3b";
const BOT_COLOR = "#3bd6d0";

const nrows = 4;
const ncols = 4;

const legalActionRegex = new RegExp("^[0-4X]{0,1}$");

const e = React.createElement;

class Cell extends React.PureComponent {

    constructor() {
        super();
        this.state = {
            content: "",
        };
    }

    render() {
        return e(
            "td",
            null,
            e(
                "input",
                {
                    disabled: this.props.guess !== null,
                    onBlur: this.onBlur.bind(this),
                    onChange: this.onChange.bind(this),
                    style: {
                        color: this.getColor(),
                    },
                    value: this.getContent(),
                },
            )
        );
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
        if (this.props.guess === null) {
            return this.state.content;
        }
        if (this.props.guess === 0) {
            return "X";
        }
        return this.props.guess;
    }

    onBlur(e) {
        if (legalActionRegex.test(this.state.content)) {
            try {
                this.props.onBlur(this.props.row, this.props.col, e);
            } catch (err) {
                console.log(err);
            }
        }
        this.setState({ content: "" });
    }

    onChange(e) {
        this.setState({ content: e.currentTarget.value });
        return this.props.onChange(this.props.row, this.props.col, e);
    }
}

class Grid extends React.PureComponent {

    constructor() {
        super();
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

    getGameState() {
        let grid = [];
        for (let r = 0; r < nrows; ++r) {
            let row = [];
            for (let c = 0; c < ncols; ++c) {
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
            scores: this.bot ? this.bot.get_scores() : [0, 0],
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
                row.push(e(
                    Cell,
                    {
                        col: c,
                        guess: guess,
                        id: `${r},${c}`,
                        onBlur: this.onCellInputBlur.bind(this),
                        onChange: this.onCellInputChange.bind(this),
                        player: player,
                        row: r,
                    },
                ));
            }
            rows.push(e("tr", null, ...row));
        }
        return e(
            "div",
            {
                className: "prophecies-game",
            },
            e(
                "table",
                {},
                e(
                    "tbody",
                    null,
                    ...rows,
                ),
            ),
            e(
                "div",
                {
                    className: "scores",
                },
                e(
                    "div",
                    {},
                    `${this.state.scores[0]} - ${this.state.scores[1]}`,
                ),
            )
        );
    }

    onCellInputBlur(row, col, e) {
        // Already made a move in this cell
        if (this.state.grid[row][col].player !== null) {
            return;
        }
        const cellValue = e.currentTarget.value;
        if (!cellValue || !cellValue.length || !legalActionRegex.test(cellValue)) {
            return;
        }
        const guess = cellValue === "X" ? 0 : parseInt(cellValue, 10);
        this.takeAction({ row, col, guess });
    }

    onCellInputChange(row, col, e) {
        if (!legalActionRegex.test(e.target.value)) {
            console.log(`${e.target.value} failed regex`);
            return;
        }
    }

    ponder() {
        if (!this.shouldPonder) {
            return;
        }
        this.bot.playout_n(1024);
        window.setTimeout(this.ponder.bind(this), 1000);
    }

    takeAction(action) {
        if (!action) {
            return;
        }
        // Throws IllegalMoveError if move is illegal
        this.bot.place(action.row, action.col, action.guess);
        this.setState(this.getGameState());
    }

    takeBotAction() {
        this.shouldPonder = false;
        this.takeAction(this.bot.get_best_action());
        this.shouldPonder = true;
    }
}

ReactDOM.render(
    e(Grid, { nrows: 4, ncols: 4 }),
    document.getElementById("prophecies-grid-container"),
);