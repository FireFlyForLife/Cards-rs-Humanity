/**
 * \file `CrsH-Frontend.js`
 * 
 * \brief this file contains the logic for visualising the Cards-rs-Humanity game.
 * \dependson `CrsH-Gameplay.js` `jquery-3.4.0.js` `signals.js` `jquery-form.js`
 */


// Variables:
//type: HashMap<cardElement, cardId
var cardElementToCardId = {}; 
//type: HashMap<cardElement, cardId
var revealedCardIdToElement = {};

$(document).ready(function() {
	$("#cardRevealing").hide();

	//Connect listeners to the gameplay events:
	connection.onGameState.add(onNewGameStateReceived);
	connection.onPlayerJoinedMatch.add(onPlayerJoined);
	connection.onPlayerLeftMatch.add(onPlayerLeft);
	connection.onEveryoneSubmittedCards.add(onEveryoneSubmittedCards);
	connection.onRevealCard.add(onRevealOthersCard);
	connection.onCzarCardChoice.add(onCzarCardChoiceReceived);
	connection.onNewRound.add(onNewRoundStarted);
	connection.onPlayerWon.add(onPlayerWon);
	connection.onNewCzar.add(onNewCzar);
	connection.onMatchHasStarted.add(onMatchHasStarted);

	//Create forms which don't redirect you to another page:
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
});

function onNewGameStateReceived(msg) {
	renderUserList();
}
function onPlayerJoined(msg) {
	renderUserList();
}
function onPlayerLeft(msg) {
	renderUserList();
}

function renderUserList(){
	var isCzar = userList.length > 0 && userList[0].id == ourSelves.id;
	$("#startGameButton").prop('disabled', !isCzar);

	$("#userList").html('');
	$.each(userList, function(i, val) {
		var czarString = val.id == czarId ? " (czar)" : "";
		var opString = i == 0 ? " (op)" : "";

		$("#userList").append(val.name + czarString + opString + "<br>");
	});
}

function onNewRoundStarted() {
	$("#cardRevealing").hide();
	$("#handOfCards").show();
	
	$(".submittedCard").removeClass("submittedCard");
	$(".selectedCard").removeClass("selectedCard");

	if(isCzar()) {
		$("#submitButton").attr("disabled", true);
	} else {
		$("#submitButton").attr("disabled", false);
	}
}

function onMatchHasStarted() {
	if(isCzar()) {
		$("#submitButton").attr("disabled", true);
	} else {
		$("#submitButton").attr("disabled", false);
	}
}

function onPlayerWon(msg) {
	var playerId = msg.playerId;

	var playerName = userList.find(function(nameIdPair, i) {
		return nameIdPair.id == playerId;
	})
	alert("player with the name: " + playerName + " has won the match!");
}

function onNewCzar(msg) {
	if(isCzar()) {
		$("#submitButton").attr("disabled", true);
	} else {
		$("#submitButton").attr("disabled", false);
	}

	renderUserList();
}

function onEveryoneSubmittedCards(msg) {
	var ids = msg.cardIds;

	$("#handOfCards").hide();
	$("#cardRevealing").show();

	$("#cardRevealing").html("");

	$("#submitButton").attr("disabled",  false);

	revealedCardIdToElement = {};
	$.each(ids, function(i, id){
		var card = document.createElement('div');
		card.classList.add("downfacingCard");
		var textNode = document.createTextNode("Click to reveal...");
		card.appendChild(textNode);
		revealedCardIdToElement[id] = card;
		
		var copyId = id;
		card.onclick = function () {
			if (everyonesSubmittedCards[copyId] == null) {
				connection.sendRevealCard(new outgoingMessages.RevealCard(copyId));
			}
		}

		$("#cardRevealing").append(card);
	});
}

function onRevealOthersCard(msg) {
	var id = msg.cardId;
	var content = msg.cardContent;

	revealedCardIdToElement[id].classList.remove("downfacingCard");
	revealedCardIdToElement[id].classList.add("revealedCard");
	revealedCardIdToElement[id].innerText = content;

	revealedCardIdToElement[id].onclick = function() {
		let everythingRevealed = false;
		$.each(everyonesSubmittedCards, function(i, val){
			everythingRevealed |= val != null;
		});
		
		if(everythingRevealed) {
			deselectCards();
			selectCard(this);
		} else {
			connection.sendRevealCard(new outgoingMessages.RevealCard(id));
		}
	}
	
}

function onCzarCardChoiceReceived(msg) {
	var cardId = msg.cardId;
	var cardElem = revealedCardIdToElement[cardId];
	if(cardElem != null) {
		cardElem.classList.add("chosenCard");
	}else{
		console.error("ERROR: Cannot find a card with the id: " + cardId);
	}
}

//From the selected card Element instance
function getOthersSubmittedCardId(textElement) {
	for (key in revealedCardIdToElement) {
		if(revealedCardIdToElement[key] == textElement) {
			return parseInt(key);
		}
	}

	return null;
}

function submitSelection() {
	var canSubmitCzarChoice = (! $.isEmptyObject(everyonesSubmittedCards) );
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
			$("#submitButton").attr("disabled",  true);
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
			var cardId = cardElementToCardId[selectedCard];
			if(cardId == null) {
				console.error("Selected card: ", selectCard, "Doesn't have an id");
				return;
			}
			selectedCard.classList.add("submittedCard");
			$("#submitButton").attr("disabled",  true);

			submitCard(cardId);
		}
	}
}

// Called from a html button
function startGame() {
    if (connection != null) {
        connection.sendStartGame();
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

function getSelectedCard() {
	var whiteCards = document.getElementsByClassName("selectedCard");
	if (whiteCards.length > 0) {
		return whiteCards[0];
	} else {
		return null;
	}
}

//overload gameplay callbacks to visualize it.
gameplayCallbacks.addWhiteCard = function(msg) {
	var text = msg.cardContent;
	var cardId = msg.cardId;
	/* Create a card like this:
		<div class="whiteCard">The text of the card</div>
	*/
	var card = document.createElement('div');
	card.classList.add("whiteCard");
	var textNode = document.createTextNode(text);
	card.appendChild(textNode);
	cardElementToCardId[card] = cardId;

	card.onclick = function () {
		if (!hasSubmittedCard) {
			deselectCards();
			selectCard(this);
		}
	}

	$("#handOfCards").append(card);
};

gameplayCallbacks.matchListReceived = function(matchList) {
	$("#matches").html("");

	$.each(matchList, function(i, matchId) { 
		var idCopy = matchId;
		var btn = $('<button/>')
			.text('Join match')
			.click(function() { 
				alert('Joining match: ' + idCopy); 
				var ajaxRequest = sendJoinMatch(new outgoingMessages.JoinMatch(idCopy));
				ajaxRequest.done(function(data) { 
					console.log("Successfully joined match!");
					//TODO: Set some html element to a happy face or something idk
					connection.connect(idCopy);
				});
				ajaxRequest.fail(function(request, status, error) {
					alert("ERROR match memes went wrong!!!. Some info: " + request.responseText + " + " + error + " + " + status);
				});
			});
		$("#matches").append(matchId).append(btn).append("<br>");
	});
}
