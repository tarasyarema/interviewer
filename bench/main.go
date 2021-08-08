package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"net/url"
	"sync"
	"sync/atomic"
	"time"

	"github.com/gorilla/websocket"
)

type Event struct {
	Username string `json:"username"`
	Session  string `json:"session"`
	Event    string `json:"event"`
	Data     string `json:"data"`
	Ts       int64  `json:"ts"`
}

type LoginMsg struct {
	Username  string `json:"username"`
	SessionID string `json:"session_id"`
}

type Range struct {
	Row    int `json:"row"`
	Column int `json:"column"`
}

type ChangeMsg struct {
	ID     *int     `json:"id"`
	Action string   `json:"action"`
	Start  Range    `json:"start"`
	End    Range    `json:"end"`
	Lines  []string `json:"lines"`
}

var addr = flag.String("addr", "localhost:1337", "http service address")

const SESSION_ID string = "__bench_test"
const USERNAME string = "__test_%d"
const N int = 100

func main() {
	flag.Parse()
	wg := sync.WaitGroup{}
	u := url.URL{Scheme: "wss", Host: *addr}

	fmt.Printf("Connection string: %s\n", u.String())

	ok := int64(0)

	for i := 0; i < N; i++ {
		i := i

		wg.Add(1)

		go func() {
			defer wg.Done()

			c, _, err := websocket.DefaultDialer.Dial(u.String(), nil)
			if err != nil {
				fmt.Printf("%d: connect error = %s\n", i, err)
				return
			}

			defer c.Close()

			data, _ := json.Marshal(LoginMsg{
				Username:  fmt.Sprintf(USERNAME, i),
				SessionID: SESSION_ID,
			})

			login := Event{
				Username: fmt.Sprintf(USERNAME, i),
				Session:  SESSION_ID,
				Event:    "login",
				Data:     string(data),
				Ts:       time.Now().UnixNano(),
			}

			if err := c.WriteJSON(login); err != nil {
				fmt.Printf("%d: write login error = %s\n", i, err)
				return
			}

			fmt.Printf("%d: logged in\n", i)

			time.Sleep(5 * time.Second)

			for j := 0; j < N; j++ {
				text := fmt.Sprintf("message (%d, %d)", i, j)

				changeData, _ := json.Marshal(ChangeMsg{
					ID:     &i,
					Action: "insert",
					Start:  Range{Row: 0, Column: 0},
					End:    Range{Row: 1, Column: 0},
					Lines:  []string{text, ""},
				})

				change := Event{
					Username: fmt.Sprintf(USERNAME, i),
					Session:  SESSION_ID,
					Event:    "change",
					Data:     string(changeData),
					Ts:       time.Now().UnixNano(),
				}

				if err := c.WriteJSON(change); err != nil {
					fmt.Printf("%d: write change error = %s\n", i, err)
					return
				}

				fmt.Printf("%d (%d): sent change, %d bytes\n", i, j, len(changeData))

				time.Sleep(100 * time.Millisecond)
			}

			time.Sleep(1 * time.Minute)

			for j := 0; j < N; j++ {
				text := fmt.Sprintf("message (%d, %d)", i, j)

				changeData, _ := json.Marshal(ChangeMsg{
					ID:     &i,
					Action: "insert",
					Start:  Range{Row: 0, Column: 0},
					End:    Range{Row: 1, Column: 0},
					Lines:  []string{text, ""},
				})

				change := Event{
					Username: fmt.Sprintf(USERNAME, i),
					Session:  SESSION_ID,
					Event:    "change",
					Data:     string(changeData),
					Ts:       time.Now().UnixNano(),
				}

				if err := c.WriteJSON(change); err != nil {
					fmt.Printf("%d: write change error = %s\n", i, err)
					return
				}

				fmt.Printf("%d (%d): sent change, %d bytes\n", i, j, len(changeData))

				time.Sleep(1 * time.Second)
			}

			time.Sleep(1 * time.Minute)

			if err := c.WriteMessage(websocket.CloseMessage, websocket.FormatCloseMessage(websocket.CloseNormalClosure, "")); err != nil {
				fmt.Printf("%d: close error = %s\n", i, err)
				return
			}

			atomic.AddInt64(&ok, 1)
		}()
	}

	wg.Wait()

	fmt.Printf("%d / %d\n", ok, N)
}
