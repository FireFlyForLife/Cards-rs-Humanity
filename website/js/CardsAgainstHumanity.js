var handOfCardsContainer = null;
//~~type: List<Pair<cardId, cardElement>>~~
//type: HashMap<cardElement, cardId>
var handOfCards = {};
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
	}
};


class ServerSocketConnection {
	socketConnection = null;
	onAddCardToHand = new signals.Signal();
	onPlayerSubmittedCard = new signals.Signal();
	onEveryoneSubmittedCards = new signals.Signal();
	onRevealCard = new signals.Signal();

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

	sendListMatches() {
		var message = {type: "listMatches"};
		var messageJson = JSON.stringify(message);

		this.socketConnection.send(messageJson);
	}

	sendJoinMatch(matchId) {
		var message = {type: "joinMatch", matchId: matchId};
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
					console.error("EveryoneSubmittedCards message received, but the 'card_content' property is not a number (or not defined)");
					break;
				}
				if(jsonData["card_id"] == null || typeof jsonData["card_id"] != "number") {
					console.error("EveryoneSubmittedCards message received, but the 'card_id' property is not a number (or not defined)");
					break;
				}

				var message = new incommingMessages.RevealCard(jsonData["card_content"], jsonData["card_id"]);
				this.onRevealCard.dispatch(message);
			break;
			default:
				console.error("Unknown message type send by server. Full JSON: " + JSON.stringify(jsonData));
			break;
		}
	}

	disconnect() {
		if (this.socketConnection != null) {
			log('Disconnecting...');
			this.socketConnection.close();
			this.socketConnection = null;
		}
	}
	connect() {
		this.disconnect();
		var wsUri = (window.location.protocol == 'https:' && 'wss://' || 'ws://') + window.location.host + '/ws/';
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
	}
	isConnected() {
		return this.socketConnection != null;
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

window.onload = function () {
	handOfCardsContainer = document.getElementById("handOfCards");
	submitButton = document.getElementById("submitButton");

	connection = new ServerSocketConnection();
	connection.connect();

	connection.onAddCardToHand.add(addWhiteCard);
	// addWhiteCard("White card from JS #1");
	// addWhiteCard("White card from JS #2");
	// addWhiteCard("White card from JS #3");
	// addWhiteCard("White card from JS #4");
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

		// var payload = JSON.stringify({ type: "submitCard", card_id: 123321 });

		// connection.socketConnection.send(payload);
		connection.sendSubmitCard(new outgoingMessages.SubmitCard(cardId));

		// $.ajax({
		//     url: 'submitCard',
		//     // dataType: 'json', //Omitted because jquery trips over itself it I put it in
		//     type: 'post',
		//     contentType: 'application/json',
		//     data: payload,
		//     processData: false,
		// })
		// .done(function( data, textStatus, jQxhr ) {
		// 	alert( "Submitted card response: " + jQxhr.status + " Data Loaded: '" + data + "'" );
		// })
		// .fail(function(request, status, error) {
		// 	alert("ERROR Submitting card. Some info: " + request.responseText + " + " + error + " + " + status);
		// })

	}
}

function newRoundStarts() {
	hasSubmittedCard = false;
	submitButton.disabled = false;
}
