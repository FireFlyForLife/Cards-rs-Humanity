var handOfCardsContainer = null;
//type: HashMap<buttonElement, matchName>
var matchList = {};
//type: HashMap<cardElement, cardId>
var handOfCards = {};
//type: Array<Player{name, id}>
var userList = [];
//type: Player{name, id}
var ourSelves = null;
var hasSubmittedCard = false;
var submitButton = null;

var connection = null;


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
	}
};

// Message Types for messages which are received by the client
var incommingMessages = {
	AddCardToHand: function(cardContent, cardId) {
		this.cardContent = cardContent;
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
	}
};

// Send a GET request to the server.
// This function is async, it will return a JQuerry Ajax object. When that request is completed, the data should contain a JSON array of strings
//
// @returns ajax request returning a String[] of all matches
function sendListMatches() {
	var req = $.ajax({
		url: 'api/list_matches',
		type: 'get',
	})
	.done(function( data, textStatus, jQxhr ) {
		console.log( "received list_matches response: " + jQxhr.status + " Data Loaded: '" + data + "'" );
	})
	.fail(function(request, status, error) {
		alert("ERROR listing all matches. Some info: " + request.responseText + " + " + error + " + " + status);
	});

	return req;
}

// send a GET request to join a match. 
// The server will automiatically disconnect you from a previous match if you were already in another match
//
// @arg joinMatch an instance of `outgoingMessages.JoinMatch`
// @returns jquerry ajax request object returning nothing on success, but an error string on failure
function sendJoinMatch(joinMatch) {
	var request = $.ajax({
		url: '/api/join/' + joinMatch.matchId,
		type: 'get',
	})
	.done(function(data, textStatus, jQxhr) {
		console.log( "joining match successful: " + jQxhr.status + " Data Loaded: '" + data + "'" );
	})
	.fail(function(request, status, error) {
		alert("ERROR joining match. Some info: " + request.responseText + " + " + error + " + " + status);
	});

	return request;
}

class ServerSocketConnection {
	socketConnection = null;
	onAddCardToHand = new signals.Signal();
	onPlayerSubmittedCard = new signals.Signal();
	onEveryoneSubmittedCards = new signals.Signal();
	onRevealCard = new signals.Signal();
	onGameState = new signals.Signal();
	onPlayerJoinedMatch = new signals.Signal();
	onPlayerLeftMatch = new signals.Signal();
	// Fired when the server has agreed that the match has started
	onMatchHasStarted = new signals.Signal();

	constructor() {
		
	}

	// @arg submitCard an instance of the type `outgoingMessages.SubmitCard`
	sendSubmitCard(submitCard) {
		var message = {type: "submitCard", card_id: submitCard.cardId};
		var messageJson = JSON.stringify(message);

		this.socketConnection.send(messageJson);
	}

	// @arg revealCard an instance of the type `outgoingMessages.RevealCard`
	sendRevealCard(revealCard) {
		var message = {type: "revealCard", card_id: revealCard.cardId};
		var messageJson = JSON.stringify(message);
		
		this.socketConnection.send(messageJson);
	}

	sendJoinMatch(matchId) {
		var message = {type: "joinMatch", matchId: matchId};
		var messageJson = JSON.stringify(message);

		this.socketConnection.send(messageJson);
	}

	sendStartGame() {
		var message = {type: "startGame"};
		var messageJson = JSON.stringify(message);

		this.socketConnection.send(messageJson);
	}

	//Message handler for socket connection.
	// @param e MessageEvent (see https://developer.mozilla.org/en-US/docs/Web/API/MessageEvent#Properties)
	parseConnectionData(e) {
		var data = e.data;
		var jsonData = JSON.parse(data);
		if(jsonData == null) {
			console.error("socket connection message is not valid json!");
			return;
		}

		var messageType = jsonData["type"];
		if(messageType == null) {
			console.error("socket connection message type is empty!");
			return;
		}

		switch(messageType) {
			case "playerSubmittedCard":
				if(jsonData["card_id"] == null || typeof jsonData["card_id"] != "number") {
					console.error("PlayerSubmittedCard message received, but the 'card_id' property is not a number (or not defined)");
					break;
				}
				if(jsonData["user_uuid"] == null || typeof jsonData["user_uuid"] != "string") {
					console.error("PlayerSubmittedCard message received, but the 'userUuid' property is not a string (or not defined)");
					break;
				}

				var message = new incommingMessages.PlayerSubmittedCard(jsonData["user_uuid"], jsonData["card_id"]);
				this.onPlayerSubmittedCard.dispatch(message);
			break;
			case "addCardToHand":
				if(jsonData["card_content"] == null || typeof jsonData["card_content"] != "string") {
					console.error("AddCardToHand message received, but the 'card_id' property is not a string (or not defined)");
					break;
				}
				if(jsonData["card_id"] == null || typeof jsonData["card_id"] != "number") {
					console.error("AddCardToHand message received, but the 'card_id' property is not a number (or not defined)");
					break;
				}

				var message = new incommingMessages.AddCardToHand(jsonData["card_content"], jsonData["card_id"]);
				this.onAddCardToHand.dispatch(message);
			break;
			case "everyoneSubmittedCards":
				if(jsonData["card_ids"] == null || !Array.isArray(jsonData["card_ids"])) {
					console.error("EveryoneSubmittedCards message received, but the 'card_ids' property is not an array (or not defined)");
					break;
				}

				var message = new incommingMessages.EveryoneSubmittedCards(jsonData["card_ids"]);
				this.onEveryoneSubmittedCards.dispatch(message);
			break;
			case "revealCard":
				if(jsonData["card_content"] == null || typeof jsonData["card_content"] != "string") {
					console.error("EveryoneSubmittedCards message received, but the 'card_content' property is not a string (or not defined)");
					break;
				}
				if(jsonData["card_id"] == null || typeof jsonData["card_id"] != "number") {
					console.error("EveryoneSubmittedCards message received, but the 'card_id' property is not a number (or not defined)");
					break;
				}

				var message = new incommingMessages.RevealCard(jsonData["card_content"], jsonData["card_id"]);
				this.onRevealCard.dispatch(message);
			break;
			case "player_left":
				if(jsonData["player_id"] == null || typeof jsonData["player_id"] != "string") {
					console.error("EveryoneSubmittedCards message received, but the 'player_id' property is not a string (or not defined)");
					break;
				}

				var message = new incommingMessages.PlayerLeftMatch(jsonData["player_id"]);
				this.onPlayerLeftMatch.dispatch(message);
			break;
			case "player_joined":
				if(jsonData["player"] == null || typeof jsonData["player"] != "object") {
					console.error("EveryoneSubmittedCards message received, but the 'player' property is not a object (or not defined)");
					break;
				}

				var message = new incommingMessages.PlayerJoinedMatch(jsonData["player"]);
				this.onPlayerJoinedMatch.dispatch(message);
			break;
			case "matchStarted":
				this.onMatchHasStarted.dispatch();
			break;
			default:
				console.error("Unknown message type send by server. Full JSON: " + JSON.stringify(jsonData));
			break;
		}
	}

	disconnect() {
		if (this.socketConnection != null) {
			console.log('Disconnecting...');
			this.socketConnection.close();
			this.socketConnection = null;
		}
	}
	connect(matchId) {
		this.disconnect();
		var wsUri = (window.location.protocol == 'https:' && 'wss://' || 'ws://') + window.location.host + '/ws/' + matchId;
		this.socketConnection = new WebSocket(wsUri);
		console.log('Connecting...');
		var self = this;
		this.socketConnection.onopen = function () {
			console.log('Connected.');
		};
		this.socketConnection.onmessage = function (e) {
			console.log('Received: ' + e.data);
			self.parseConnectionData(e);
		};
		this.socketConnection.onclose = function () {
			console.log('Disconnected.');
			this.socketConnection = null;
		};
		this.socketConnection.onerror = function(error) {
			console.error("WebSocket error observed:", error);
		}
	}
	isConnected() {
		return this.socketConnection != null && this.socketConnection.readyState == 1;
	}
}



function deselectCards() {
	var whiteCards = document.getElementsByClassName("whiteCard");
	for (var i = 0; i < whiteCards.length; i++) {
		whiteCards[i].classList.remove("selectedCard");
	}
}

function selectCard(whiteCard) {
	whiteCard.classList.add("selectedCard");
}

function addWhiteCard(msg) {
	var text = msg.cardContent;
	var cardId = msg.cardId;
	/* Create a card like this:
		<div class="whiteCard">The text of the card</div>
	*/
	var card = document.createElement('div');
	card.classList.add("whiteCard");
	var textNode = document.createTextNode(text);
	card.appendChild(textNode);
	// handOfCards.push({id: cardId, text: textNode});
	handOfCards[card] = cardId;

	card.onclick = function () {
		if (!hasSubmittedCard) {
			deselectCards();
			selectCard(this);
		}
	}

	handOfCardsContainer.appendChild(card);
}

function newGameStateReceived(data) {
	var jsonData = JSON.parse(data);
	if(jsonData == null) {
		console.error("newGameStateReceived message is not valid json!");
		return;
	}

	console.log("New gamestate received!" + jsonData);
	if(jsonData["other_players"] == null || !Array.isArray(jsonData["other_players"])) {
		console.error("GameState message received, but the 'other_players' property is not an array (or not defined)");
		return;
	}
	if(jsonData["our_player"] == null || typeof jsonData["our_player"] != "object") {
		console.error("GameState message received, but the 'our_player' property is not an object (or not defined)");
		return;
	}
	if(jsonData["hand_of_cards"] == null || !Array.isArray(jsonData["hand_of_cards"])) {
		console.error("GameState message received, but the 'hand_of_cards' property is not an array (or not defined)");
		return;
	}
	if(jsonData["czar"] == null || typeof jsonData["czar"] != 'string') {
		console.error("GameState message received, but the 'czar' property is not a string (or not defined)");
		return;
	}
	if(jsonData["started"] == null || typeof jsonData["started"] != 'boolean') {
		console.error("GameState message received, but the 'started' property is not a boolean (or not defined)");
		return;
	}

	var message = new incommingMessages.GameState(jsonData["other_players"], jsonData["our_player"], jsonData["hand_of_cards"], jsonData["czar"], jsonData["started"]);

	ourSelves = message.ourPlayer;

	userList = [];
	$.each(message.otherPlayers, function(i, val) {
		userList.push(val);
	});
	renderUserList();
}

function renderUserList(){
	var isCzar = userList.length > 0 && userList[0].id == ourSelves.id;
	$("#startGameButton").prop('disabled', !isCzar);

	$("#userList").html('');
	$.each(userList, function(i, val) {
		if(i == 0)
			$("#userList").append(val.name + " (czar)<br>");
		else
			$("#userList").append(val.name + "<br>");
	});
}

function refreshMatchList() {
	var ajaxReq = sendListMatches();
	ajaxReq.done(function( data ) {
		//alert( "refreshing match list with " + data );
		var json = JSON.parse(data);
		if(json == null || !Array.isArray(json)) {
			console.error("refreshMatchList response was received, but the data is not valid json. or json is not an array. Ignoring response");
			return;
		}
		
		matchList = {};

		$("#matches").html("");
		$.each(json, function(i, val) { 
			var valCopy = val;
			var btn = $('<button/>')
				.text('Join match')
				.click(function() { 
					alert('Joining match: ' + valCopy); 
					var ajaxRequest = sendJoinMatch(new outgoingMessages.JoinMatch(valCopy));
					ajaxRequest.done(function(data) { 
						console.log("Successfully joined match!");
						newGameStateReceived(data);
						//TODO: Set some html element to a happy face or something idk
						connection.connect(valCopy);
					});
					ajaxRequest.fail(function(request, status, error) {
						alert("ERROR match memes went wrong!!!. Some info: " + request.responseText + " + " + error + " + " + status);
					});
				});
			$("#matches").append(val).append(btn).append("<br>");
		});
	})
}

function startGame() {
	connection.sendStartGame();
}

function playerLeft(message) {
	var index = -1;
	$.each(userList, function(i, val) {
		if(val.id == message.otherPlayerId)
			index = i;
	});
	if (index != -1) {
		userList.splice(index, 1);
	}

	renderUserList();
}

function playerJoined(message) {
	userList.push(message.otherPlayer);

	renderUserList();
}

function matchHasStarted() {
	alert("Match has been started!");
}

window.onload = function () {
	handOfCardsContainer = document.getElementById("handOfCards");
	submitButton = document.getElementById("submitButton");

	connection = new ServerSocketConnection();

	connection.onAddCardToHand.add(addWhiteCard);
	connection.onGameState.add(newGameStateReceived);
	connection.onPlayerLeftMatch.add(playerLeft)
	connection.onPlayerJoinedMatch.add(playerJoined);
	connection.onMatchHasStarted.add(matchHasStarted);
	
	$('#loginForm').ajaxForm({
		success: function() {
			alert("Thank you for your login!");
		},
		error: function(request, status, error) {
			alert("ERROR on login. Some info: " + request.responseText + " + " + error + " + " + status);
		}
	});
	$('#registerForm').ajaxForm({
		success: function() {
			alert("Thank you for your registering!");
		},
		error: function(request, status, error) {
			alert("ERROR onregister. Some info: " + request.responseText + " + " + error + " + " + status);
		}
	});
}

function getSelectedCard() {
	var whiteCards = document.getElementsByClassName("selectedCard");
	if (whiteCards.length > 0) {
		return whiteCards[0];
	} else {
		return null;
	}
}

function submitSelection() {
	if (hasSubmittedCard) {
		alert("You have already submitted your card!");
		return;
	}

	var selectedCard = getSelectedCard();
	if (selectedCard === null) {
		alert("No card is selected! please select one by clicking on it!");
	} else {
		var cardId = handOfCards[selectedCard];
		if(cardId == null) {
			console.error("Selected card: ", selectCard, "Doesn't have an id");
			return;
		}
		selectedCard.classList.add("submittedCard");
		hasSubmittedCard = true;
		submitButton.disabled = true;

		connection.sendSubmitCard(new outgoingMessages.SubmitCard(cardId));
	}
}

function newRoundStarts() {
	hasSubmittedCard = false;
	submitButton.disabled = false;
}
