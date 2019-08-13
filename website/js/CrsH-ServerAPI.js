/**
 * \file `CrsH-ServerAPI.js`
 * 
 * \brief This file contains functions for interacting with the Cards-rs-humanity server.
 * \dependson `jquery-3.4.0.js` `signals.js`
 */


// Message Types for messages which can be send from the client
var outgoingMessages = {
	// @arg cardId the cardId from your hand to submit for this round
	SubmitCard: function(cardId) {
		this.cardId = cardId;
	},
	// @arg cardId the card czar reveals a card. 
	RevealCard: function(cardId) {
		this.cardId = cardId;
	},
	JoinMatch: function(matchId) {
		this.matchId = matchId;
	},
	ListMatches: function() {
	},
	StartMatch: function() {
	},
	// @arg cardId the card id of the card which is the best
	CzarCardChoice: function(cardId) {
		this.cardId = cardId;
	}
};

// Message Types for messages which are received by the client
var incommingMessages = {
	AddCardToHand: function(cardContent, cardId) {
		this.cardContent = cardContent;
		this.cardId = cardId;
	}, 
	RemoveCardFromHand: function(cardId) {
		this.cardId = cardId;
	},
	//This message is also send for the current player. along with all other players
	PlayerSubmittedCard: function(userUuid, cardId) {
		this.userUuid = userUuid;
		this.cardId = cardId;
	},
	// @arg matches is List<Pair<MatchId, String>>
	ListMatches: function(matches) {
		this.matches = matches;
	},
	PlayerJoinedMatch(matchId, playerId, playerName){
		this.matchId = matchId;
		this.playerId = playerId;
		this.playerName = playerName;
	},
	// @arg cardIds a array of card ids, of all cards submitted this round
	EveryoneSubmittedCards: function(cardIds) {
		this.cardIds = cardIds;
	},
	// @arg cardContent the text of the white card which gets revealed.
	// @arg cardId the card the czar revealed. 
	RevealCard: function(cardContent, cardId) {
		this.cardContent = cardContent;
		this.cardId = cardId;
	},
	// @arg otherPlayers Array<{name, id}> an array of objects, each object will have a field "name" and "id"
	// @arg ourPlayer {name, id} an object with the fields "name" and "id"
	// @arg handOfCards Array<String> an array of cards contents
	// @arg czar Number a id of the player which is the czar (aka host)
	GameState: function(otherPlayers, ourPlayer, handOfCards, czar, gameStarted) {
		this.otherPlayers = otherPlayers;
		this.ourPlayer = ourPlayer;
		this.handOfCards = handOfCards;
		this.czar = czar;
		this.gameStarted = gameStarted;
	},
	// @arg otherPlayer {name, id} an object with the fields "name" and "id"
	PlayerJoinedMatch: function(otherPlayer) {
		this.otherPlayer = otherPlayer;
	},
	// @arg otherPlayerId the id of the player leaving the match
	PlayerLeftMatch: function(otherPlayerId) {
		this.otherPlayerId = otherPlayerId;
	},
	// Fired when the server has agreed that the match has started
	MatchHasStarted: function() {
	},
	// @arg cardId the card id of the card which is the best
	CzarCardChoice: function(cardId) {
		this.cardId = cardId;
	},
	// @arg playerId the playerId of the player who won the match
	PlayerWon: function(playerId) {
		this.playerId = playerId;
	},
	PlayerRoundWin: function(playerId) {
		this.playerId = playerId;
	},
	// A new round has started
	NewRound: function() {
	},
	// @arg czar the id of the player who is now the czar
	NewCzar: function(czar) {
		this.czar = czar;
	},
	// @arg deckName the name of the deck
	// @arg blackCards an array of question cards
	// @arg whiteCards an array of response cards
	CardDeck: function(deckName, blackCards, whiteCards){
		this.deckName = deckName;
		this.blackCards = blackCards;
		this.whiteCards = whiteCards;
	},
	NewBlackCard: function(cardId, cardContent) {
		this.cardId = cardId;
		this.cardContent = cardContent;
	}
};

// check if the property named `key` exists and is of the type `type`.
// In the case this is not true, a error message will be printed to the console.
//
// @arg json
// @arg key the name of the property which should exist.
// @arg type a string of the type that key should have. supports all `typeof` return values and 'array'
// @arg additionalMessage OPTIONAL a message which will be prepended to the end of the error message, often used to give context to the error log
//
// @returns bool if the property exists and is the correct type
function validateJsonProperty(json, key, type, additionalMessage) {
	var val = json[key];
	if (val == null) {
		console.error(additionalMessage + " The value of key: '"+key+"' doesn't exist!");
		return false;
	}
	
	var valType = Array.isArray(val) ? 'array' : typeof val;
	if(valType != type) {
		console.error(additionalMessage + " The value of key: '" +key+"' should have the type: '"+type+"' but is instead: '"+valType+"'");
		return false;
	}

	return true;
}

// \returns incommingMessages.GameState if valid, or null if the json was not a valid GameState.
function _parseJsonToGameState(data) {
	try{
		var jsonData = JSON.parse(data);
	}catch{
		console.error("newGameStateReceived message is not valid json!");
		return null;
	}
	if(jsonData == null) {
		console.error("newGameStateReceived message is a null value! should be a object");
		return null;
	}
	if(!validateJsonProperty(jsonData, 'other_players', 'array', 	"GameState message received,")) { return null; }
	if(!validateJsonProperty(jsonData, 'our_player', 	'object', 	"GameState message received,")) { return null; }
	if(!validateJsonProperty(jsonData, 'hand_of_cards', 'array', 	"GameState message received,")) { return null; }
	if(!validateJsonProperty(jsonData, 'czar', 			'number', 	"GameState message received,")) { return null; }
	if(!validateJsonProperty(jsonData, 'started', 		'boolean', 	"GameState message received,")) { return null; }

	var message = new incommingMessages.GameState(jsonData["other_players"], jsonData["our_player"], jsonData["hand_of_cards"], jsonData["czar"], jsonData["started"]);
	return message;
}

// \returns incommingMessages.CardDeck if valid, or null if the json was not a valid CardDeck.
function _parseJsonToCardDeck(data) {
	try{
		var jsonData = JSON.parse(data);
	}catch{
		console.error("GetCardDeck message is not valid json!");
		return null;
	}
	if(jsonData == null) {
		console.error("GetCardDeck message is a null value! should be a object");
		return null;
	}

	if(!validateJsonProperty(jsonData, 'deck_name', 'string', 'GetCardDeck message received,')) { return null; }
	if(!validateJsonProperty(jsonData, 'black_cards', 'array', 'GetCardDeck message received,')) { return null; }
	if(!validateJsonProperty(jsonData, 'white_cards', 'array', 'GetCardDeck message received,')) { return null; }

	var message = new incommingMessages.CardDeck(jsonData['deck_name'], jsonData['black_cards'], jsonData['white_cards']);
	return message;
}

// Send a GET request to the server.
// This function is async, it will return a JQuerry Ajax object. When that request is completed, the data should contain a JSON array of strings
//
// @returns ajax request returning a String[] of all matches
function sendListMatches() {
	var req = $.ajax({
		url: 'api/list_matches',
		type: 'get',
	});

	return req;
}

// send a GET request to join a match. 
// The server will automiatically disconnect you from a previous match if you were already in another match
//
// @arg joinMatch an instance of `outgoingMessages.JoinMatch`
// @arg afterParsed OPTIONAL 	a function(gameState), when defined will be called when the ajaxRequest has finished and the message received has been decoded. 
// 								The gamestate argument is of type: `incommingMessages.GameState`.
//								This function is called before the global `connection.onGameState` signal is dispatched.
//
// @returns jquerry ajax request object returning a json object convertable to `incommingMessages.GameState` on success, but an error string on failure
function sendJoinMatch(joinMatch, afterParsed) {
	var request = $.ajax({
		url: '/api/join/' + joinMatch.matchId,
		type: 'get',
	});

	var afterParsedCopy = afterParsed;
	request.done(function( data, textStatus, jQxhr ) {
		var newGameState = _parseJsonToGameState(data);
		if(newGameState != null) {
			if(afterParsedCopy != undefined) {
				afterParsedCopy(newGameState);
			}
			//TODO: This is quite dirty, clean it up
			connection.onGameState.dispatch(newGameState);
		}
	});

	return request;
}

// send a GET request to get all cards in a given deck.
// You are required to be logged in before this the server will accept the request.
//
// @arg deckName the name of the deck to query
// @arg afterParsed OPTIONAL 	a `function(cardDeck)`, when defined will be called when the ajaxRequest has finished and the message received has been decoded. 
// 								The cardDeck argument is of type: `incommingMessages.CardDeck`.
//
// @returns jquerry ajax request object returning a json object convertable to `incommingMessages.CardDeck` on success, but an error string on failure
function sendGetCardDeck(deckName, afterParsed) {
	var request = $.ajax({
		url: '/api/cards/'+deckName,
		type: 'get',
	});

	if(afterParsed != undefined) {
		var afterParsedCopy = afterParsed;
		request.done(function( data, textStatus, jQxhr ) {
			var newGameState = _parseJsonToCardDeck(data);
			if(newGameState != null) {
				afterParsedCopy(newGameState);
			}
		});
	}

	return request;
}

// send a POST request to add a card to a given deck.
// You are required to be logged in before this the server will accept the request.
// 
// @arg deckName the name of the deck to add the card to
// @arg cardContent the text contents of the card
// @arg isWhiteCard set to false if this is a white 'response' card. set to false if this is a black `question` card.
//
// @returns jquery ajax request object returning the cardId on success, but an error string on failure.
function sendAddCard(deckName, cardContent, isWhiteCard) {
	var urlPrefix = isWhiteCard ? '/api/add/w/' : '/api/add/b/';

	var request = $.ajax({
		url: urlPrefix+deckName,
		type: 'post',
		data: cardContent,
	});

	return request;
}

// send a POST request to remove a card from a given deck
// You are required to be logged in before this the server will accept the request.
//
// @arg deckName
// @arg cardId
// 
// @returns jquery ajax request object returning nothing on success, but an error string on failure.
function sendDeleteCard(deckName, cardId) {
	var request = $.ajax({
		url: '/api/del/' + deckName + '/' + cardId,
		type: 'post',
	})

	return request;
}

class ServerSocketConnection {
	constructor() {
		this._socketConnection = null;

		this.onAddCardToHand = new signals.Signal();
		this.onRemoveCardFromHand = new signals.Signal();
		this.onPlayerSubmittedCard = new signals.Signal();
		this.onEveryoneSubmittedCards = new signals.Signal();
		this.onRevealCard = new signals.Signal();
		this.onGameState = new signals.Signal();
		this.onPlayerJoinedMatch = new signals.Signal();
		this.onPlayerLeftMatch = new signals.Signal();
		// Fired when the server has agreed that the match has started
		this.onMatchHasStarted = new signals.Signal();
		this.onCzarCardChoice = new signals.Signal();
		this.onPlayerWon = new signals.Signal();
		this.onNewRound = new signals.Signal();
		this.onNewCzar = new signals.Signal();
		this.onNewBlackCard = new signals.Signal();
		this.onPlayerRoundWin = new signals.Signal();
	}

	// @arg submitCard an instance of the type `outgoingMessages.SubmitCard`
	sendSubmitCard(submitCard) {
		var message = {type: "submitCard", card_id: submitCard.cardId};
		var messageJson = JSON.stringify(message);

		this._socketConnection.send(messageJson);
	}

	// @arg czarCardChoice an instance of the type `outgoingMessages.CzarCardChoice`
	sendCzarCardChoice(czarCardChoice) {
		var message = {type: "czarChoice", card_id: czarCardChoice.cardId};
		var messageJson = JSON.stringify(message);

		this._socketConnection.send(messageJson);
	}

	// @arg revealCard an instance of the type `outgoingMessages.RevealCard`
	sendRevealCard(revealCard) {
		var message = {type: "revealCard", card_id: revealCard.cardId};
		var messageJson = JSON.stringify(message);
		
		this._socketConnection.send(messageJson);
	}

	sendStartGame() {
		var message = {type: "startGame"};
		var messageJson = JSON.stringify(message);

		this._socketConnection.send(messageJson);
	}

	//Message handler for socket connection.
	// @param e MessageEvent (see https://developer.mozilla.org/en-US/docs/Web/API/MessageEvent#Properties)
	parseConnectionData(e) {
		var data = e.data;
		try{
			var jsonData = JSON.parse(data);
		}catch{
			console.error("socket connection message is not valid json!");
			return;
		}
		if(jsonData == null) {
			console.error("socket connection message is a null value! It should be a object");
			return;
		}
		
		var messageType = jsonData["type"];
		if(messageType == null) {
			console.error("socket connection message type is empty!");
			return;
		}

		switch(messageType) {
			case "playerSubmittedCard":
				if(!validateJsonProperty(jsonData, 'card_id', 'number', "PlayerSubmittedCard message received,")) { return; }
				if(!validateJsonProperty(jsonData, 'user_uuid', 'string', "PlayerSubmittedCard message received,")) { return; }

				var message = new incommingMessages.PlayerSubmittedCard(jsonData["user_uuid"], jsonData["card_id"]);
				this.onPlayerSubmittedCard.dispatch(message);
			break;
			case "addCardToHand":
				if(!validateJsonProperty(jsonData, 'card_content', 'string', "AddCardToHand message received,")) { return; }
				if(!validateJsonProperty(jsonData, 'card_id', 'number', "AddCardToHand message received,")) { return; }

				var message = new incommingMessages.AddCardToHand(jsonData["card_content"], jsonData["card_id"]);
				this.onAddCardToHand.dispatch(message);
			break;
			case "removeCard":
				if(!validateJsonProperty(jsonData, 'card_id', 'number', "RemoveCardFromhand message received,")) { return; }

				var message = new incommingMessages.RemoveCardFromHand(jsonData["card_id"]);
				this.onRemoveCardFromHand.dispatch(message);
			break;
			case "everyone_submitted":
				if(!validateJsonProperty(jsonData, 'card_ids', 'array', "EveryoneSubmittedCards message received,")) { return; }

				var message = new incommingMessages.EveryoneSubmittedCards(jsonData["card_ids"]);
				this.onEveryoneSubmittedCards.dispatch(message);
			break;
			case "revealCard":
				if(!validateJsonProperty(jsonData, 'card_content', 'string', "RevealCard message received,")) { return; }
				if(!validateJsonProperty(jsonData, 'card_id', 'number', "RevealCard message received,")) { return; }

				var message = new incommingMessages.RevealCard(jsonData["card_content"], jsonData["card_id"]);
				this.onRevealCard.dispatch(message);
			break;
			case "player_left":
				if(!validateJsonProperty(jsonData, 'player_id', 'number', "PlayerLeft message received,")) { return; } 

				var message = new incommingMessages.PlayerLeftMatch(jsonData["player_id"]);
				this.onPlayerLeftMatch.dispatch(message);
			break;
			case "player_joined":
				//TODO: Also check contents of player object
				if(!validateJsonProperty(jsonData, 'player', 'object', "PlayerJoined message received,")) { return; } 

				var message = new incommingMessages.PlayerJoinedMatch(jsonData["player"]);
				this.onPlayerJoinedMatch.dispatch(message);
			break;
			case "matchStarted":
				this.onMatchHasStarted.dispatch();
			break;
			case "czar_choice":
				if(!validateJsonProperty(jsonData, 'card_id', 'number', "CzarChoice message received,")) { return; } 

				var message = new incommingMessages.CzarCardChoice(jsonData["card_id"]);
				this.onCzarCardChoice.dispatch(message);
			break;
			case "playerWon":
				if(!validateJsonProperty(jsonData, 'player_id', 'number', "PlayerWon message received,")) { return; }
				
				var message = new incommingMessages.PlayerWon(jsonData["player_id"]);
				this.onPlayerWon.dispatch(message);
			break;
			case "roundWon":
				if(!validateJsonProperty(jsonData, 'player_id', 'number', "PlayerRoundWin message received,")) { return; }

				var message = new incommingMessages.PlayerRoundWin(jsonData["player_id"]);
				this.onPlayerRoundWin.dispatch(message);
			break;
			case "newRound":
				this.onNewRound.dispatch();
			break;
			case "newCzar": 
				if(!validateJsonProperty(jsonData, 'czar', 'number', "NewCzar message received,")) { return; }

				var message = new incommingMessages.NewCzar(jsonData["czar"]);
				this.onNewCzar.dispatch(message);
			break;
			case "newBlack":
				if(!validateJsonProperty(jsonData, 'card_id', 'number', "NewBlackCard message received,")) { return; }
				if(!validateJsonProperty(jsonData, 'card_content', 'string', "NewBlackCard message received,")) { return; }

				var message = new incommingMessages.NewBlackCard(jsonData["card_id"], jsonData["card_content"]);
				this.onNewBlackCard.dispatch(message);
			break;
			default:
				console.error("Unknown message type send by server. Full JSON: " + JSON.stringify(jsonData));
			break;
		}
	}

	disconnect() {
		if (this._socketConnection != null) {
			console.log('Disconnecting...');
			this._socketConnection.close();
			this._socketConnection = null;
		}
	}
	connect(matchId) {
		this.disconnect();
		var wsUri = (window.location.protocol == 'https:' && 'wss://' || 'ws://') + window.location.host + '/ws/' + matchId;
		this._socketConnection = new WebSocket(wsUri);
		console.log('Connecting...');
		var self = this;
		this._socketConnection.onopen = function () {
			console.log('Connected.');
		};
		this._socketConnection.onmessage = function (e) {
			console.log('Received: ' + e.data);
			self.parseConnectionData(e);
		};
		this._socketConnection.onclose = function () {
			console.log('Disconnected.');
			this.socketConnection = null;
		};
		this._socketConnection.onerror = function(error) {
			console.error("WebSocket error observed:", error);
		};
	}
	isConnected() {
		return this._socketConnection != null && this._socketConnection.readyState == 1;
	}
}
