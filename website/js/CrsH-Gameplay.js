/**
 * \file `CrsH-Gameplay.js`
 * 
 * \brief This file contains the clientside gameplay logic. But doesn't handle the frontend visualisation, that is reserved for `CrsH-Frontend.js`
 * \dependson `CrsH-ServerAPI.js` `jquery-3.4.0.js` `signals.js`
 */

//type: Array<matchId>
var matchList = [];
//type Array<cardId>
var handOfCards = [];
//type: Array<Player{name, id}>
var userList = [];
//type: Player{name, id}
var ourSelves = null;
//type: string
var czarId = null;
//type: HashMap<cardId, cardContent>
var everyonesSubmittedCards = {};

var hasSubmittedCard = false;
var everyoneHasSubmittedCards = false;
var hasSubmittedCzarChoice = false;

//type: class ServerSocketConnection
var connection = null;


//TODO: When I feel like it make this the matchListReceived callback use the actual incommingMessage type. 

//callbacks:
var gameplayCallbacks = {
	// signature: function(msg: incommingMessages.AddCardToHand)
	addWhiteCard: null,

	// signature: function(matchList: Array<matchId>)
	matchListReceived: null,

	// signature: function()
	matchHasStarted: null
}

function submitCard(cardId) {
	if(!hasSubmittedCard) {
		connection.sendSubmitCard(new outgoingMessages.SubmitCard(cardId));	

		hasSubmittedCard = true;
	}
}
function isCzar() {
	return ourSelves != null && ourSelves.id == czarId;
}
function czarChooseCard(cardId) {

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
		
		matchList = json;

		if(gameplayCallbacks.matchListReceived != null) {
			gameplayCallbacks.matchListReceived(matchList);
		}
	});
}

function _addWhiteCard(msg) {
	var cardId = msg.cardId;

	handOfCards.push(cardId);

	if(gameplayCallbacks.addWhiteCard != null) {
		gameplayCallbacks.addWhiteCard(msg);
	}
}

function _newGameStateReceived(gameStateMessage) {
	ourSelves = gameStateMessage.ourPlayer;

	userList = [];
	$.each(gameStateMessage.otherPlayers, function(i, val) {
		userList.push(val);
	});
}

function _playerLeft(message) {
	var index = -1;
	$.each(userList, function(i, val) {
		if(val.id == message.otherPlayerId)
			index = i;
	});
	if (index != -1) {
		userList.splice(index, 1);
	}
}

function _playerJoined(message) {
	userList.push(message.otherPlayer);
}

function _matchHasStarted() {
	// alert("Match has been started!");
	if(gameplayCallbacks.matchHasStarted != null) {
		gameplayCallbacks.matchHasStarted();
	}
}

function _everyoneSubmittedCards(msg) {
	var ids = msg.cardIds;

	everyoneHasSubmittedCards = true;

	everyonesSubmittedCards = {};
	$.each(ids, function(i, id){
		everyonesSubmittedCards[id] = null;
	});
}

function _revealOthersCard(msg) {
	var id = msg.cardId;
	var content = msg.cardContent;

	if(everyonesSubmittedCards[id] == null) {
		everyonesSubmittedCards[id] = content;
	}
}

function _czarCardChoiceReceived(msg) {
	window.setTimeout(function() {
		//This should be a seperate event I think
		newRoundStarts();
	}, 2000);
}

$(document).ready(function () {
	$("#cardRevealing").hide();

	connection = new ServerSocketConnection();

	connection.onAddCardToHand.add(_addWhiteCard);
	connection.onGameState.add(_newGameStateReceived);
	connection.onPlayerLeftMatch.add(_playerLeft);
	connection.onPlayerJoinedMatch.add(_playerJoined);
	connection.onMatchHasStarted.add(_matchHasStarted);
	connection.onEveryoneSubmittedCards.add(_everyoneSubmittedCards);
	connection.onRevealCard.add(_revealOthersCard);
	connection.onCzarCardChoice.add(_czarCardChoiceReceived);
});

function newRoundStarts() {
	hasSubmittedCard = false;
	everyoneHasSubmittedCards = false;
	hasSubmittedCzarChoice = false;
	everyonesSubmittedCards = {};
	$("#cardRevealing").hide();
	$("#handOfCards").show();
}