/**
 * \file `CrsH-Gameplay.js`
 * 
 * \brief This file contains the clientside gameplay logic. But doesn't handle the frontend visualisation, that is reserved for `CrsH-Frontend.js`
 * \dependson `CrsH-ServerAPI.js` `jquery-3.4.0.js` `signals.js`
 */

//type: HashMap<buttonElement, matchName>
var matchList = {};
//type: HashMap<cardElement, cardId>
var handOfCards = {};
//type: Array<Player{name, id}>
var userList = [];
//type: Player{name, id}
var ourSelves = null;
//type: HashMap<cardId, {cardElement, content}>
var othersSubmittedCards = {};

var handOfCardsContainer = null;
var hasSubmittedCard = false;
var everyoneHasSubmittedCards = false;
var hasSubmittedCzarChoice = false;
var submitButton = null;

//type: class ServerSocketConnection
var connection = null;


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
			$("#userList").append(val.name + " (op)<br>");
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
	});
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
	// alert("Match has been started!");
}

function everyoneSubmittedCards(msg) {
	var ids = msg.cardIds;

	$("#handOfCards").hide();
	$("#cardRevealing").show();

	everyoneHasSubmittedCards = true;
	submitButton.disabled = false;

	$.each(ids, function(i, id){
		var card = document.createElement('div');
		card.classList.add("downfacingCard");
		var textNode = document.createTextNode("Click to reveal...");
		card.appendChild(textNode);
		othersSubmittedCards[id] = {textElement: card, content: null};
		
		var copyId = id;
		card.onclick = function () {
			if (othersSubmittedCards[copyId].content == null) {
				connection.sendRevealCard(new outgoingMessages.RevealCard(copyId));
			}
		}

		$("#cardRevealing").append(card);
	});
}

function revealOthersCard(msg) {
	var id = msg.cardId;
	var content = msg.cardContent;

	if(othersSubmittedCards[id].content == null) {
		othersSubmittedCards[id].content = content;

		othersSubmittedCards[id].textElement.classList.remove("downfacingCard");
		othersSubmittedCards[id].textElement.classList.add("revealedCard");
		othersSubmittedCards[id].textElement.innerText = content;

		othersSubmittedCards[id].textElement.onclick = function() {
			let everythingRevealed = false;
			$.each(othersSubmittedCards, function(i, val){
				everythingRevealed |= val.content != null;
			});
			
			if(everythingRevealed) {
				deselectCards();
				selectCard(this);
			} else {
				connection.sendRevealCard(new outgoingMessages.RevealCard(id));
			}
		}
	}
}

function czarCardChoiceReceived(msg) {
	var cardId = msg.cardId;
	var cardElemObj = othersSubmittedCards[cardId];
	if(cardElemObj == null) {
		console.error("ERROR: Cannot find a card with the id: " + cardId);
		return;
	}
	cardElemObj.textElement.classList.add("chosenCard");

	window.setTimeout(function() {
		//This should be a seperate event I think
		newRoundStarts();
	}, 2000);
}

$(document).ready(function () {
	handOfCardsContainer = document.getElementById("handOfCards");
	submitButton = document.getElementById("submitButton");

	$("#cardRevealing").hide();

	connection = new ServerSocketConnection();

	connection.onAddCardToHand.add(addWhiteCard);
	connection.onGameState.add(newGameStateReceived);
	connection.onPlayerLeftMatch.add(playerLeft);
	connection.onPlayerJoinedMatch.add(playerJoined);
	connection.onMatchHasStarted.add(matchHasStarted);
	connection.onEveryoneSubmittedCards.add(everyoneSubmittedCards);
	connection.onRevealCard.add(revealOthersCard);
	connection.onCzarCardChoice.add(czarCardChoiceReceived);
});

function getSelectedCard() {
	var whiteCards = document.getElementsByClassName("selectedCard");
	if (whiteCards.length > 0) {
		return whiteCards[0];
	} else {
		return null;
	}
}

//From the selected card Element instance
function getOthersSubmittedCardId(textElement) {
	for (key in othersSubmittedCards) {
		if(othersSubmittedCards[key].textElement == textElement) {
			return parseInt(key);
		}
	}

	return null;
}

function submitSelection() {
	var canSubmitCzarChoice = (! $.isEmptyObject(othersSubmittedCards) );
	if(canSubmitCzarChoice) {
		if(/* !isCzar() */ false) {
			return;
		}

		var selectedCard = getSelectedCard();
		if (selectedCard === null) {
			alert("No card is selected! please select one by clicking on it!");
		} else {
			var cardId = getOthersSubmittedCardId(selectedCard);
			if(cardId == null) {
				console.error("Selected card: ", selectCard, "Doesn't have an id");
				return;
			}

			connection.sendCzarCardChoice(new outgoingMessages.CzarCardChoice(cardId));

			selectedCard.classList.add("submittedCard");
			submitButton.disabled = true;
		}
	} else {
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
}

function newRoundStarts() {
	hasSubmittedCard = false;
	everyoneHasSubmittedCards = false;
	hasSubmittedCzarChoice = false;
	submitButton.disabled = false;
	othersSubmittedCards = {};
	$("#cardRevealing").hide();
	$("#handOfCards").show();
}
