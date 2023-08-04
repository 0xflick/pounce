import bB from './assets/bB.svg'
import bK from './assets/bK.svg'
import bN from './assets/bN.svg'
import bP from './assets/bP.svg'
import bQ from './assets/bQ.svg'
import bR from './assets/bR.svg'
import wB from './assets/wB.svg'
import wK from './assets/wK.svg'
import wN from './assets/wN.svg'
import wP from './assets/wP.svg'
import wQ from './assets/wQ.svg'
import wR from './assets/wR.svg'

import moveSound from './assets/Move.mp3'
import captureSound from './assets/Capture.mp3'
import { Howl } from 'howler';

import { CSSProperties, ReactNode, useEffect, useRef, useState } from 'react'
import { DndProvider, XYCoord, useDrag, useDragLayer, useDrop } from 'react-dnd'
import './App.css'
import { HTML5Backend, getEmptyImage } from 'react-dnd-html5-backend'

const startBoardState: { [loc in Location]?: PieceType } = {
  "a1": "wR",
  "b1": "wN",
  "c1": "wB",
  "d1": "wK",
  "e1": "wQ",
  "f1": "wB",
  "g1": "wN",
  "h1": "wR",
  "a2": "wP",
  "b2": "wP",
  "c2": "wP",
  "d2": "wP",
  "e2": "wP",
  "f2": "wP",
  "g2": "wP",
  "h2": "wP",
  "a8": "bR",
  "b8": "bN",
  "c8": "bB",
  "d8": "bK",
  "e8": "bQ",
  "f8": "bB",
  "g8": "bN",
  "h8": "bR",
  "a7": "bP",
  "b7": "bP",
  "c7": "bP",
  "d7": "bP",
  "e7": "bP",
  "f7": "bP",
  "g7": "bP",
  "h7": "bP",
}

const columns = "abcdefgh".split("")

const pieces = {
  "bB": bB,
  "bK": bK,
  "bN": bN,
  "bP": bP,
  "bQ": bQ,
  "bR": bR,
  "wB": wB,
  "wK": wK,
  "wN": wN,
  "wP": wP,
  "wQ": wQ,
  "wR": wR,
}

type Location =
  | "a1"
  | "a2"
  | "a3"
  | "a4"
  | "a5"
  | "a6"
  | "a7"
  | "a8"
  | "b1"
  | "b2"
  | "b3"
  | "b4"
  | "b5"
  | "b6"
  | "b7"
  | "b8"
  | "c1"
  | "c2"
  | "c3"
  | "c4"
  | "c5"
  | "c6"
  | "c7"
  | "c8"
  | "d1"
  | "d2"
  | "d3"
  | "d4"
  | "d5"
  | "d6"
  | "d7"
  | "d8"
  | "e1"
  | "e2"
  | "e3"
  | "e4"
  | "e5"
  | "e6"
  | "e7"
  | "e8"
  | "f1"
  | "f2"
  | "f3"
  | "f4"
  | "f5"
  | "f6"
  | "f7"
  | "f8"
  | "g1"
  | "g2"
  | "g3"
  | "g4"
  | "g5"
  | "g6"
  | "g7"
  | "g8"
  | "h1"
  | "h2"
  | "h3"
  | "h4"
  | "h5"
  | "h6"
  | "h7"
  | "h8"

type PieceType =
  | "bB"
  | "bK"
  | "bN"
  | "bP"
  | "bQ"
  | "bR"
  | "wB"
  | "wK"
  | "wN"
  | "wP"
  | "wQ"
  | "wR"

type PieceProps = {
  kind: PieceType
  loc: Location
}

function Piece({ kind, loc }: PieceProps) {
  const ref = useRef<HTMLDivElement>(null)

  const [{ isDragging }, drag, preview] = useDrag(() => ({
    type: "piece",
    item: { piece: kind, loc: loc } as PieceItem,
    collect: monitor => ({
      isDragging: !!monitor.isDragging()
    })
  }), [kind, loc])

  const [{ x, y }, setState] = useState({ x: 0, y: 0 })
  const transform = `translate(${x}px, ${y}px)`

  useEffect(() => { preview(getEmptyImage(), { captureDraggingState: true }) })

  function onMouseEvent(e: any) {
    if (ref.current === undefined || ref.current === null) {
      return
    }
    const rect = ref.current.getBoundingClientRect()
    let tX = (e.clientX - (rect.left + rect.right) / 2)
    let tY = (e.clientY - (rect.top + rect.bottom) / 2)
    setState({ x: tX, y: tY })
  }


  useEffect(() => {
    if (isDragging && (x != 0 || y != 0)) {
      setState({ x: 0, y: 0 })
    }
  }, [isDragging])

  return (
    <>
      <div ref={ref} onMouseDown={onMouseEvent} onMouseUp={() => (setState({ x: 0, y: 0 }))} style={{
        transform,
      }}>
        <img ref={drag} className="piece" style={{
          opacity: isDragging ? 0 : 1,
        }} src={pieces[kind]} />
      </div >
    </>
  )
}

type SquareProps = {
  color: "white" | "black"
  location: Location,
  setLoc: (loc: Location, piece: PieceType | undefined) => void,
  children: ReactNode
}

const blackColor = { backgroundColor: "#90A1AC" }
const whiteColor = { backgroundColor: "#DFE3E6" }

interface PieceItem {
  piece: PieceType
  loc: Location
}

function Square({ color, location, children, setLoc }: SquareProps) {
  const [{ isOver }, drop] = useDrop(() => ({
    accept: "piece",
    drop: ({ piece, loc }: PieceItem) => {
      if (loc === location) {
        return
      }
      setLoc(loc, undefined)
      setLoc(location, piece)
    },
    collect: monitor => ({
      isOver: !!monitor.isOver() && monitor.getItem().loc != location
    }),
  }), [location])

  const style: CSSProperties = {
    ...(color == "black" ? blackColor : whiteColor),
    boxSizing: "border-box",
    boxShadow: isOver ? "inset 0 0 0 6px #bbb" : ""
  };

  return <div ref={drop} className="square" style={style}>
    {children}
  </div>
}

function Board() {
  const [boardState, setBoardState] = useState(startBoardState)
  const moveFx = new Howl({ src: moveSound })
  const captureFx = new Howl({ src: captureSound })

  function setLoc(loc: Location, piece: PieceType | undefined) {
    setBoardState(prevBoard => {
      if (piece != undefined) {
        captureFx.stop()
        moveFx.stop()
        if (prevBoard[loc] === undefined || !(loc in prevBoard)) {
          moveFx.play()
        } else {
          captureFx.play()
        }
      }

      return {
        ...prevBoard,
        [loc]: piece,
      }
    })
  }

  return (
    <DndProvider backend={HTML5Backend}>
      <div className="board">
        {[...Array(8)].map((_, r) => {
          return (
            <div key={r.toString()} className="row">
              {[...Array(8)].map((_, c) => {
                const loc = (columns[c] + (8 - r)) as Location
                const color = c % 2 === r % 2 ? "white" : "black"

                return (
                  <Square key={loc} color={color} location={loc} setLoc={setLoc}>
                    {boardState[loc] && (
                      <Piece kind={boardState[loc]!} loc={loc} />
                    )
                    }
                  </Square>
                )
              })}
            </div>
          )
        })}
        <CustomDragLayer />
      </div>
    </DndProvider>
  )
}

function getItemStyles(
  initialOffset: XYCoord | null,
  currentOffset: XYCoord | null,
) {
  if (!initialOffset || !currentOffset) {
    return {
      display: 'none',
    }
  }

  let { x, y } = currentOffset

  const transform = `translate(${x}px, ${y}px)`
  return {
    transform,
    WebkitTransform: transform,
  }
}

const layerStyles: CSSProperties = {
  position: 'fixed',
  pointerEvents: 'none',
  zIndex: 100,
  left: 0,
  top: 0,
  width: '100%',
  height: '100%',
}

function CustomDragLayer() {
  const { item, isDragging, initialOffset, currentOffset } =
    useDragLayer((monitor) => ({
      item: monitor.getItem() as PieceItem,
      isDragging: monitor.isDragging(),
      initialOffset: monitor.getInitialClientOffset(),
      currentOffset: monitor.getSourceClientOffset(),
    }))

  function renderItem() {
    return (
      <img className="piece" src={pieces[item.piece]} />
    )
  }

  if (!isDragging) {
    return null
  }
  return (
    <div style={layerStyles}>
      <div style={getItemStyles(initialOffset, currentOffset)}>
        {renderItem()}
      </ div>
    </div>
  )
}

function App() {

  return (
    <>
      <Board />
    </>
  )
}

export default App
