package model

type mtime int32

type Route struct {
	ID          string
	Name        string
	ProductType int16
	Message     string
	Direction   string
}

type Station struct {
	ID         string
	Name       string
	Departures []*Connection
	Arrivals   []*Connection
	Lat        float32
	Lon        float32
}

type Connection struct {
	Route       *Route
	From        *Station
	To          *Station
	Departure   StopInfo
	Arrival     StopInfo
	Message     string
	Cancelled   bool
	ProductType int16
}

type StopInfo struct {
	Scheduled      mtime
	Delay          int16
	ScheduledTrack string
	ProjectedTrack string
}

type Distribution struct {
	Histogram []float32
	Start     mtime
}
